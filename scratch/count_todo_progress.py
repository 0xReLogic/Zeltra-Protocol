import re

def main():
    todo_path = "/home/azureuser/crypto/todo.md"
    with open(todo_path, "r") as f:
        lines = f.readlines()
        
    current_section = "Header"
    section_stats = {}
    
    # Simple state machine to parse sections
    for line in lines:
        # Check for section headers (e.g. ## P0 - Blocker Keamanan)
        heading_match = re.match(r"^##\s+(.*)", line)
        if heading_match:
            current_section = heading_match.group(1).strip()
            section_stats[current_section] = {"checked": 0, "unchecked": 0}
            continue
            
        # Match [x] or [ ]
        if "[x]" in line:
            if current_section in section_stats:
                section_stats[current_section]["checked"] += 1
        elif "[ ]" in line:
            if current_section in section_stats:
                section_stats[current_section]["unchecked"] += 1
                
    # Print a markdown table with the results
    print(f"{'Section / Phase':<50} | {'Checked':<8} | {'Unchecked':<9} | {'% Complete':<10}")
    print("-" * 50 + "-+----------+-----------+-----------")
    
    total_checked = 0
    total_unchecked = 0
    
    for section, stats in section_stats.items():
        checked = stats["checked"]
        unchecked = stats["unchecked"]
        total = checked + unchecked
        pct = (checked / total * 100) if total > 0 else 0
        
        total_checked += checked
        total_unchecked += unchecked
        
        # Only show sections that have tasks
        if total > 0:
            print(f"{section:<50} | {checked:<8} | {unchecked:<9} | {pct:.1f}%")
            
    total_all = total_checked + total_unchecked
    total_pct = (total_checked / total_all * 100) if total_all > 0 else 0
    print("-" * 50 + "-+----------+-----------+-----------")
    print(f"{'TOTAL PROGRESS':<50} | {total_checked:<8} | {total_unchecked:<9} | {total_pct:.1f}%")

if __name__ == "__main__":
    main()
