//! Background synchronization task for OFAC SDN dataset (DEC-026 Layer 3)
//!
//! vile/ofac-sdn-list updates daily via GitHub Action at 06:00 UTC.
//! This sync task triggers 1 hour later (07:00 UTC) to fetch the latest
//! sanctioned EVM addresses and update the in-memory cache without restarting the node.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const PRIMARY_SDN_URL: &str =
    "https://raw.githubusercontent.com/vile/ofac-sdn-list/main/sdn.json";
pub const FALLBACK_SDN_URL: &str =
    "https://github.com/vile/ofac-sdn-list/releases/latest/download/sdn.json";

/// Compute duration until the next 07:00 UTC
pub fn duration_until_next_0700_utc() -> Duration {
    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let seconds_in_day = 86400;
    let current_day_secs = now_secs % seconds_in_day;
    let target_secs = 7 * 3600; // 07:00 UTC (1 hour after GitHub 06:00 UTC action)

    let wait_secs = if current_day_secs < target_secs {
        target_secs - current_day_secs
    } else {
        (seconds_in_day - current_day_secs) + target_secs
    };

    Duration::from_secs(wait_secs)
}

/// Attempt to fetch and parse the latest SDN dataset from GitHub
pub async fn fetch_and_update_sdn(client: &reqwest::Client) -> Result<usize, String> {
    // 1. Try primary URL
    let body = match client
        .get(PRIMARY_SDN_URL)
        .header("User-Agent", "Zeltra-Relayer-Node/1.0")
        .timeout(Duration::from_secs(30))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => resp.text().await.map_err(|e| e.to_string())?,
        _ => {
            // 2. Fallback to releases asset URL
            let resp = client
                .get(FALLBACK_SDN_URL)
                .header("User-Agent", "Zeltra-Relayer-Node/1.0")
                .timeout(Duration::from_secs(30))
                .send()
                .await
                .map_err(|e| format!("Fallback fetch failed: {}", e))?;

            if !resp.status().is_success() {
                return Err(format!(
                    "Both primary and fallback SDN fetch failed. HTTP status: {}",
                    resp.status()
                ));
            }
            resp.text().await.map_err(|e| e.to_string())?
        }
    };

    let parsed = crate::validation::parse_sdn_evm_addresses(&body)
        .map_err(|e| format!("Failed to parse SDN JSON: {}", e))?;

    // Sanity check: must contain at least 50 EVM addresses to avoid corrupt/empty lists
    if parsed.len() < 50 {
        return Err(format!(
            "Parsed SDN list too small ({} addresses), rejecting update to prevent cache purge",
            parsed.len()
        ));
    }

    let count = crate::validation::update_sanctioned_addresses(parsed);
    Ok(count)
}

/// Spawn the background recurring sync task
pub fn spawn_sanctions_sync_task() -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        println!("SANCTIONS SYNC: Worker initialized. Embedded sdn.json is active.");

        // Wait 10 seconds after startup before initial optional background refresh
        tokio::time::sleep(Duration::from_secs(10)).await;
        match fetch_and_update_sdn(&client).await {
            Ok(count) => {
                println!(
                    "SANCTIONS SYNC: Initial refresh successful. Active sanctioned EVM addresses: {}",
                    count
                );
            }
            Err(e) => {
                println!(
                    "SANCTIONS SYNC WARNING: Initial refresh skipped ({}), using embedded dataset & on-chain oracle fallback.",
                    e
                );
            }
        }

        loop {
            let wait_duration = duration_until_next_0700_utc();
            let hours = wait_duration.as_secs() / 3600;
            let minutes = (wait_duration.as_secs() % 3600) / 60;
            println!(
                "SANCTIONS SYNC: Next scheduled refresh at 07:00 UTC (in {}h {}m)",
                hours, minutes
            );

            tokio::time::sleep(wait_duration).await;

            println!("SANCTIONS SYNC: Triggering scheduled 07:00 UTC daily refresh...");
            match fetch_and_update_sdn(&client).await {
                Ok(count) => {
                    println!(
                        "SANCTIONS SYNC: Daily update successful. Active sanctioned EVM addresses: {}",
                        count
                    );
                }
                Err(e) => {
                    eprintln!(
                        "SANCTIONS SYNC ERROR: Scheduled update failed: {}. Continuing with existing cache.",
                        e
                    );
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_duration_until_0700_utc_within_bounds() {
        let dur = duration_until_next_0700_utc();
        assert!(dur.as_secs() > 0);
        assert!(dur.as_secs() <= 86400);
    }
}
