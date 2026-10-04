**2022/534**
(PDF)
Last updated: 2024-03-14

**On the Adaptive Security of the Threshold BLS Signature Scheme**
Renas Bacho, Julian Loss

**Category:** Foundations

---

## Ringkasan

Threshold signatures merupakan komponen krusial dalam banyak protokol terdistribusi. Sebagaimana ditunjukkan oleh Cachin, Kursawe, dan Shoup (PODC '00), skema dengan unique signatures sangat penting karena memungkinkan implementasi distributed coin flipping secara efisien tanpa asumsi timing. Ini menjadikan mereka sebagai building block yang ideal untuk asynchronous consensus protocols. Threshold BLS signature dari Boldyreva (PKC '03) bersifat unique dan sangat ringkas, namun sayangnya hanya terbukti aman dalam setting statis di mana adversary harus melakukan corrupt pada semua party sebelum protokol dimulai.

Dalam karya ini, penulis menunjukkan bahwa limitasi ini bersifat inherent — dengan membuktikan impossibility result untuk adaptively secure threshold BLS signatures di plain model. Mereka juga menunjukkan bahwa situasinya bisa diperbaiki di random oracle model, di mana mereka membangun threshold BLS signature pertama yang adaptively secure. Scheme mereka berbasis BLS signature dan hanya menambahkan overhead faktor konstan dibandingkan dengan protokol asli (yang statically secure).

## Konsep Kunci

### Threshold Signatures
- Memungkinkan sekelompok party untuk secara bersama-sama menandatangani pesan
- Membutuhkan threshold t dari n party untuk menghasilkan signature yang valid
- Digunakan dalam protokol terdistribusi untuk koordinasi dan consensus

### Unique Signatures
- Suatu signature scheme bersifat unique jika hanya ada satu signature valid untuk setiap pesan
- Penting untuk distributed coin flipping dan asynchronous consensus protocols
- Referensi: Cachin, Kursawe, dan Shoup (PODC '00)

### Adaptive Security
- Model keamanan di mana adversary dapat melakukan corrupt pada party secara dinamis selama eksekusi protokol
- Lebih realistis namun lebih sulit dicapai daripada static security
- Paper ini menunjukkan bahwa threshold BLS tidak bisa mencapai adaptive security di plain model

## Kontribusi

1. **Impossibility Result**: Menunjukkan bahwa adaptively secure threshold BLS signatures tidak mungkin dicapai di plain model tanpa asumsi tambahan

2. **Hasil Positif di ROM**: Membangun threshold BLS signature pertama yang adaptively secure di random oracle model

3. **Efisiensi**: Scheme yang diusulkan mempertahankan keringkasan scheme Boldyreva asli dengan hanya penambahan overhead faktor konstan

## Relevansi dengan Nimbus

Paper ini sangat relevan dengan mekanisme threshold BLS signing Nimbus. Protokol ini menggunakan threshold BLS scheme (t, n) di mana:
- Leader + guardian (3 dari 5) harus berkolaborasi untuk menghasilkan BLS signature yang valid
- Signature tersebut digunakan sebagai kredensial untuk private withdrawal

Impossibility result menunjukkan bahwa implementasi Nimbus sebaiknya:
- Menggunakan random oracle model (via hash function) untuk adaptive security, ATAU
- Menerima asumsi static security (adversary melakukan corrupt sebelum protokol dimulai)

Ini merupakan pertimbangan penting untuk security model dari Nimbus relayer cluster.

## Referensi

- Cachin, Kursawe, Shoup (PODC '00) - Random oracles and distributed coin flipping
- Boldyreva (PKC '03) - Threshold BLS signature scheme