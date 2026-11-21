// i18n: English is the source language; Turkish comes from the TR dictionary.
// t(text, lang)  -> translated string
// tn(text, lang, vars) -> translated string with {placeholder} substitution

export type Lang = "en" | "tr";

export const LANGS: { value: Lang; label: string }[] = [
  { value: "tr", label: "Türkçe" },
  { value: "en", label: "English" },
];

const TR: Record<string, string> = {
  // --- General / navigation ---
  "Settings": "Ayarlar",
  "General": "Genel",
  "Hardware": "Donanım",
  "Tools": "Araçlar",
  "Compression": "Sıkıştırma",
  "About": "Hakkında",
  "Dark": "Koyu",
  "Light": "Açık",
  "Font scale": "Yazı ölçeği",
  "Larger interface": "Daha büyük arayüz",
  "Interface language": "Arayüz dili",
  "Turkish": "Türkçe",
  "English": "İngilizce",
  "Work mode": "Çalışma biçimi",
  "Basic behaviors that determine how files are processed.":
    "Dosyaların nasıl işleneceğini belirleyen temel davranışlar.",
  "Simple mode": "Basit mod",
  "Manage advanced options automatically in the background.":
    "Gelişmiş seçenekleri arka planda otomatik yönet.",
  "Notify when done": "Bitince bildir",
  "Show a desktop notification when the job finishes.":
    "İş tamamlandığında masaüstü bildirimi göster.",
  "Check tools on startup": "Açılışta araçları denetle",
  "Scans FFmpeg version and encoder list on startup. When off, use Refresh on the Tools tab.":
    "FFmpeg sürümünü ve kodlayıcı listesini açılışta tarar. Kapalıyken bu taramayı Araçlar sekmesindeki Yenile düğmesi yapar.",
  "Security and verification": "Güvenlik ve doğrulama",
  "Options that protect your original files.": "Orijinal dosyalarınızı koruyan seçenekler.",
  "Keep larger output": "Büyük çıktıyı koru",
  "Copy the original if compression does not shrink the file.":
    "Sıkıştırma dosyayı küçültmezse orijinali kopyala.",
  "Keep metadata": "Meta verileri koru",
  "Preserve EXIF, title and tag info when possible.":
    "EXIF, başlık ve etiket bilgilerini mümkünse koru.",
  "Keep modification time": "Değiştirilme tarihini koru",
  "Match the output file's mtime to the source.":
    "Çıktı dosyasının tarihini kaynakla eşleştir.",
  "Verify output": "Çıktıyı doğrula",
  "Check that the encoded file can be opened.":
    "Kodlama sonrası dosyanın açılabildiğini kontrol et.",
  "Dry run": "Deneme çalışması",
  "Show only the plan and estimate without writing any files.":
    "Hiçbir dosya yazmadan yalnızca planı ve tahmini göster.",
  "Resource usage and filters": "Kaynak kullanımı ve filtreler",
  "Concurrent workers": "Eşzamanlı işçi",
  "Smallest file (KB)": "En küçük dosya (KB)",
  "Largest file (MB, 0 = unlimited)": "En büyük dosya (MB, 0 = sınırsız)",
  "Exclude patterns (comma separated)": "Hariç tutulacak desenler (virgülle ayırın)",
  "Portable setup": "Taşınabilir kurulum",
  "Reset to defaults": "Varsayılan ayarlara dön",
  "All settings will revert to defaults. Continue?":
    "Tüm ayarlar varsayılan değerlere dönecek. Devam edilsin mi?",
  "Settings reset to defaults": "Ayarlar varsayılana döndürüldü",

  // --- Hardware ---
  "Detects your system and recommends the best encoder.":
    "Sisteminizi algılar ve en uygun kodlayıcıyı önerir.",
  "Refresh": "Yenile",
  "CPU": "İşlemci",
  "physical": "fiziksel",
  "logical cores": "mantıksal çekirdek",
  "Recommended workers": "Önerilen işçi",
  "Architecture": "Mimari",
  "Memory": "Bellek",
  "Available": "Kullanılabilir",
  "Operating system": "İşletim sistemi",
  "Recommended encoder": "Önerilen kodlayıcı",
  "Graphics cards": "Ekran kartları",
  "Detected via DXGI; software adapters are filtered out.":
    "DXGI üzerinden algılanır; yazılım sürücüleri elenir.",
  "No hardware GPU found; CPU will be used.":
    "Donanım ekran kartı bulunamadı; CPU kullanılacak.",
  "software adapter(s) ignored.": "yazılım adaptörü yok sayıldı.",
  "Hardware acceleration": "Donanım hızlandırma",
  "Use GPU encoding": "GPU kodlamayı kullan",
  "Speeds up video jobs if NVENC / AMF / QSV is available.":
    "NVENC / AMF / QSV varsa video işlerini hızlandırır.",
  "Allow AV1 encoder": "AV1 kodlayıcıya izin ver",
  "Allow GPU AV1 encoders (NVENC/AMF/QSV).":
    "GPU AV1 kodlayıcıları (NVENC/AMF/QSV) kullanmaya izin verir.",
  "Encoder preference": "Kodlayıcı tercihi",
  "Quality": "Kalite",
  "Speed": "Hız",
  "Balanced": "Dengeli",
  "Test status": "Test durumu",
  "Test encoders": "Kodlayıcıları test et",
  "Encoder test": "Kodlayıcı testi",
  "Recommended: {x}": "Önerilen: {x}",
  "none": "yok",
  "CPU benchmark": "İşlemci ölçümü",
  "Measure your CPU's real speed with a short compression test.":
    "Kısa bir sıkıştırma testiyle işlemcinizin gerçek hızını ölçün.",
  "Start benchmark": "Ölçümü başlat",
  "Apply {n} worker suggestion": "{n} işçi önerisine uy",
  "Hardware info not loaded yet.": "Donanım bilgisi henüz yüklenmedi.",
  "Detecting hardware and tools…": "Donanım ve araçlar algılanıyor…",
  "Unknown CPU": "Bilinmeyen işlemci",

  // --- Tools ---
  "Install or update FFmpeg and helper tools with one click.":
    "FFmpeg ve yardımcı araçları tek tıkla kurun ya da güncelleyin.",
  "Refresh all": "Tümünü yenile",
  "Video/audio engine ready": "Video/ses motoru hazır",
  "FFmpeg must be installed": "FFmpeg kurulmalı",
  "Portable": "Taşınabilir",
  "{n} encoders detected": "{n} kodlayıcı algılandı",
  "FFmpeg is the heart of video and audio compression. Without it these files are only copied.":
    "FFmpeg, video ve ses sıkıştırmanın kalbidir. Kurulmazsa bu dosyalar yalnızca kopyalanır.",
  "The sha256 checksum is verified during download and files are extracted to the tools/bin folder next to the app.":
    "İndirme sırasında sha256 özeti doğrulanır ve dosyalar uygulamanın yanındaki tools/bin klasörüne çıkarılır.",
  "Tools folder": "Araç klasörü",
  "Not found on system": "Sistemde bulunamadı",
  "Installed": "Kurulu",
  "Required": "Gerekli",
  "Optional": "İsteğe bağlı",
  "Unknown": "Bilinmiyor",
  "Installing…": "Kuruluyor…",
  "Update": "Güncelle",
  "Reinstall": "Yeniden kur",
  "Up to date": "Güncel",
  "Installed (System)": "Kurulu (Sistem)",
  "Installed (Portable)": "Kurulu (Taşınabilir)",
  "{name} status refreshed": "{name} durumu yenilendi",
  "One-click install": "Tek tıkla kur",
  "Installed together with FFmpeg.": "FFmpeg ile birlikte kurulur.",
  "Optional external tool. Found automatically if installed; not downloaded separately.":
    "İsteğe bağlı harici araç. Kuruluysa otomatik bulunur; ayrıca indirilmez.",
  "Show in folder": "Klasörde göster",
  "License note": "Lisans notu",
  "The FFmpeg \"essentials\" build provided by gyan.dev contains libx264/libx265.":
    "gyan.dev tarafından sağlanan FFmpeg \"essentials\" derlemesi libx264/libx265 içerir.",
  "If you plan commercial distribution, use your own FFmpeg build or review the license terms.":
    "Ticari dağıtım planlıyorsanız kendi FFmpeg derlemenizi kullanın veya lisans koşullarını inceleyin.",
  "Starting installation…": "Kurulum başlatılıyor…",
  "Ready": "Hazır",
  "Tool status refreshed": "Araç durumu yenilendi",
  "Hardware info updated": "Donanım bilgileri güncellendi",

  // --- Compression settings ---
  "Advanced per-type options. The lossy slider on the main screen scales these automatically.":
    "Tür bazlı gelişmiş seçenekler. Ana ekrandaki kayıplılık oranı bu değerleri otomatik ölçekler.",
  "Images": "Görseller",
  "Image": "Görsel",
  "Image format": "Görsel biçimi",
  "Max resolution (MP, 0 = unlimited)": "En büyük çözünürlük (MP, 0 = sınırsız)",
  "Quality (0 = automatic)": "Kalite (0 = otomatik)",
  "Longest side scale (%)": "En uzun kenar ölçeği (%)",
  "Convert PNG to JPEG": "PNG dosyalarını JPEG'e çevir",
  "Produces smaller JPEG when there is no transparency.":
    "Saydamlık yoksa daha küçük JPEG üretir.",
  "Convert BMP to JPEG": "BMP dosyalarını JPEG'e çevir",
  "BMP is uncompressed, so this yields large savings.":
    "BMP zaten sıkıştırılmadığı için büyük kazanç sağlar.",
  "GIF and animation": "GIF ve animasyon",
  "GIF processing": "GIF işleme",
  "Max FPS": "En yüksek FPS",
  "Max width (px, 0 = automatic)": "En büyük genişlik (px, 0 = otomatik)",
  "Infinite loop": "Sonsuz döngü",
  "If off, the animation plays only once.": "Kapalıysa animasyon yalnızca bir kez oynar.",
  "Video": "Video",
  "Codec": "Kodek",
  "Resolution": "Çözünürlük",
  "Scale (%)": "Ölçek (%)",
  "Audio codec": "Ses kodeği",
  "Encoder tune": "Kodlayıcı ayarı",
  "Film": "Film",
  "Animation": "Animasyon",
  "Preserve grain": "Grain koru",
  "10-bit encoding": "10-bit kodlama",
  "Reduces banding; some older players do not support it.":
    "Bantlanmayı azaltır; bazı eski oynatıcılar desteklemez.",
  "Denoise": "Gürültü azaltma",
  "Better compression for grainy videos.": "Grenli videolarda daha iyi sıkıştırma sağlar.",
  "Fast start for web": "Web için hızlı başlangıç",
  "Moves the moov atom to the start of the file.":
    "moov atomunu dosyanın başına taşır.",
  "Keep subtitles": "Altyazıları koru",
  "Add compatible subtitles to the output.": "Uyumlu altyazıları çıktıya ekle.",
  "Audio": "Ses",
  "Normalize audio": "Sesi normalize et",
  "Bring different files to a similar loudness level.":
    "Farklı dosyaları benzer ses seviyesine getir.",
  "Convert to mono": "Mono'ya çevir",
  "Halves size if a single channel is enough.":
    "Tek kanal yeterliyse boyutu yarıya indirir.",
  "Not yet supported types": "Henüz desteklenmeyen türler",
  "This version compresses image, video, audio, GIF and text files. The following types are currently copied to the destination unchanged:":
    "Bu sürüm görsel, video, ses, GIF ve metin dosyalarını sıkıştırır. Aşağıdaki türler şimdilik değiştirilmeden hedef klasöre kopyalanır:",
  "PDF (Ghostscript/qpdf) and office documents (in-package XML compression) engines will be added in the next version. Archive deep processing and target-size/SSIM smart mode will come in the same stage.":
    "PDF için Ghostscript/qpdf, ofis belgeleri için paket içi XML sıkıştırma motoru bir sonraki sürümde eklenecek. Arşiv içini işleme ve hedef boyut/SSIM tabanlı akıllı mod da aynı aşamada gelecek.",
  "Office documents": "Ofis belgeleri",
  "Archives": "Arşivler",
  "Fonts": "Yazı tipleri",

  // --- About ---
  "Shrink your files with a single lossy setting. Processes images, video, audio, GIF, PDF, office documents, archives, text, SVG and fonts in one pass. Picks the encoder suited to your hardware and installs required tools with one click.":
    "Dosyalarınızı tek bir kayıplılık ayarıyla küçültür. Görseller, video, ses, GIF, PDF, ofis belgeleri, arşivler, metin, SVG ve fontları tek geçişte işler. Donanımınıza uygun kodlayıcıyı seçer, gerekli araçları tek tıkla kurar.",
  "Version": "Sürüm",
  "Run mode": "Çalışma modu",
  "Application folder": "Uygulama klasörü",
  "Settings file": "Ayarlar dosyası",
  "Temporary files": "Geçici dosyalar",
  "Logs": "Günlükler",
  "Portability": "Taşınabilirlik",
  "The app installs nothing and uses no registry. Settings, tools and logs are kept in the application folder. Copy the folder to a USB drive and run with the same settings on another computer.":
    "Uygulama hiçbir yere kurulum yapmaz ve kayıt defteri kullanmaz. Ayarlar, araçlar ve günlükler uygulama klasöründe tutulur. Klasörü bir USB belleğe kopyalayarak aynı ayarlarla başka bir bilgisayarda çalıştırabilirsiniz.",
  "Open application folder": "Uygulama klasörünü aç",
  "Show settings": "Ayarları göster",
  "Thanks": "Teşekkürler",
  "Thanks to the FFmpeg, 7-Zip, Tauri, Svelte and Rust communities. This app uses open-source components; license details are in the third-party notices in the distribution folder.":
    "FFmpeg, 7-Zip, Tauri, Svelte ve Rust topluluklarına. Bu uygulama açık kaynak bileşenler kullanır; lisans ayrıntıları dağıtım klasöründeki üçüncü taraf bildirimlerinde yer alır.",

  // --- Main screen ---
  "Shrink files to the max with a single setting":
    "Dosyaları tek ayarla maksimum düzeyde küçült",
  "One setting. Every file. Smallest size.": "Tek ayar. Her dosya. En küçük boyut.",
  "video · audio · GIF · image · PDF · office · archive · text · font":
    "video · ses · GIF · görsel · PDF · ofis · arşiv · metin · font",
  "Font": "Yazı tipi",
  "Compress": "Sıkıştır",
  "Toggle theme": "Temayı değiştir",
  "Open FFmpeg status and tool setup": "FFmpeg durumunu ve araç kurulumunu açar",
  "Detected GPU and recommended encoder": "Algılanan ekran kartı ve önerilen kodlayıcı",
  "CPU cores and recommended worker count":
    "İşlemci çekirdekleri ve önerilen eşzamanlı işçi sayısı",
  "Total and available memory": "Toplam ve kullanılabilir bellek",
  "FFmpeg ready": "FFmpeg hazır",
  "No GPU": "GPU yok",
  "Application could not start.": "Uygulama başlatılamadı.",
  "Retry": "Yeniden dene",
  "Folders": "Klasörler",
  "Every file in the source folder is written to the destination, keeping the folder structure.":
    "Kaynak klasördeki her dosya, klasör yapısı korunarak hedefe yazılır.",
  "Swap": "Yer değiştir",
  "Swap source and destination": "Kaynak ve hedefi yer değiştir",
  "Source": "Kaynak",
  "Destination": "Hedef",
  "Folder to compress…": "Sıkıştırılacak klasör…",
  "Output folder…": "Çıktı klasörü…",
  "Browse": "Gözat",
  "Select source folder": "Kaynak klasörü seç",
  "Select destination folder": "Hedef klasörü seç",
  "Scan": "Tara",
  "Counts files by type and calculates total size":
    "Dosyaları türlerine göre sayar ve toplam boyutu hesaplar",
  "Open output": "Çıktıyı aç",
  "Opens destination folder in Explorer": "Hedef klasörü Dosya Gezgini'nde açar",
  "files": "dosya",
  "skipped": "atlandı",
  "Scan summary:": "Tarama:",
  "Compression ratio": "Sıkıştırma oranı",
  "A single slider that scales image quality, video CRF, audio bitrate and GIF settings together.":
    "Tek kaydırıcı; görsel kalite, video CRF değeri, ses bit hızı ve GIF ayarlarını birlikte ölçekler.",
  "Drag right for smaller files, left for higher quality":
    "Sağa çektikçe daha küçük dosyalar; sola çektikçe daha yüksek kalite",
  "Lossy": "Kayıplılık",
  "Plan:": "Plan:",
  "Lossless mode: On": "Kayıpsız mod: Açık",
  "Lossless mode: Off": "Kayıpsız mod: Kapalı",
  "3 extra presets appear in lossless mode.": "Kayıpsız modda 3 ek preset görünür.",
  "Lossless presets": "Kayıpsız presetleri",
  "All run lossless (lossy=0); the difference is how much power goes into compression and the quality balance. Hover a preset to see details in the info bar below.":
    "Tümü kayıpsız (lossy=0) çalışır; fark sıkıştırma için harcanan güç ve kalite dengesidir. Fareyi presetin üzerine getirince alt bilgi çubuğunda detay görünür.",
  "Deep analysis on, original format, lossless GIF, video CRF 15 (10-bit), quality-first encoder":
    "Derin analiz açık, orijinal biçim, GIF kayıpsız, video CRF 15 (10-bit), kalite öncelikli kodlayıcı",
  "Automatic format, automatic GIF, video CRF 15 (10-bit), balanced encoder":
    "Otomatik biçim, GIF otomatik, video CRF 15 (10-bit), dengeli kodlayıcı",
  "PNG→JPEG, GIF→MP4, video H.264 CRF 15 (8-bit), speed-first encoder, verification off":
    "PNG→JPEG, GIF→MP4, video H.264 CRF 15 (8-bit), hız öncelikli kodlayıcı, doğrulama kapalı",
  "Automatic format, lossless GIF, video H.264 CRF 15 (8-bit), speed-first encoder, verification off":
    "Otomatik biçim, GIF kayıpsız, video H.264 CRF 15 (8-bit), hız öncelikli kodlayıcı, doğrulama kapalı",
  "Apply {name} profile": "{name} profilini uygula",
  "File types": "Dosya türleri",
  "If none are selected, all types are processed. Selected types are included.":
    "Hiçbiri seçilmezse tüm türler işlenir. İşaretledikleriniz dahil edilir.",
  "All": "Tümü",
  "Process {name} files": "{name} dosyalarını işle",
  "Automatic (by source)": "Otomatik (kaynağa göre)",
  "Keep original": "Orijinali koru",
  "Automatic": "Otomatik",
  "Keep": "Koru",
  "Copy": "Kopyala",
  "Image quality (0 = automatic)": "Görsel kalitesi (0 = otomatik)",
  "GIF max FPS": "GIF en yüksek FPS",
  "Optimize as GIF": "GIF olarak optimize et",
  "Animated WebP": "Animasyonlu WebP",
  "Convert to MP4": "MP4'e çevir",
  "Max resolution (MP)": "En büyük çözünürlük (MP)",
  "Advanced": "Gelişmiş",
  "Simple": "Basit",
  "ready": "hazır",
  "missing": "eksik",
  "Open destination folder": "Hedef klasörü aç",
  "Keep defaults unless needed.": "Gerekmedikçe varsayılanları koruyun.",
  "Image and GIF": "Görsel ve GIF",
  "Quality, resolution, animation": "Kalite, çözünürlük, animasyon",
  "Video and audio": "Video ve ses",
  "Resolution, FPS, audio bitrate": "Çözünürlük, FPS, ses bit hızı",
  "GPU encoding": "GPU kodlama",
  "Use NVENC / AMF / QSV if available.": "NVENC / AMF / QSV varsa kullan.",
  "Reduces banding.": "Bantlanmayı azaltır.",
  "Balances audio level.": "Ses seviyesini dengeler.",
  "Advanced filters": "Gelişmiş filtreler",
  "Exclusion, size limit, worker count": "Hariç tutma, boyut sınırı, işçi sayısı",
  "No, write for real": "Hayır, gerçekten yaz",
  "Yes, plan only": "Evet, yalnızca planla",
  "Patterns to exclude": "Hariç tutulacak desenler",
  "Recommended workers: {n}": "Önerilen işçi: {n}",
  "Hardware settings": "Donanım ayarları",
  "Live progress": "Canlı ilerleme",
  "Preparing…": "Hazırlanıyor…",
  "Last job: {in} → {out}": "Son iş: {in} → {out}",
  "Ready. Select a source and destination folder.":
    "Hazır. Bir kaynak ve hedef klasör seçin.",
  "Processed": "İşlenen",
  "Savings": "Kazanç",
  "Saved": "Kaydedilen",
  "Duration": "Süre",
  "Folder": "Klasör",
  "Live log": "Canlı günlük",
  "Clear": "Temizle",
  "No log yet. Use “Scan” to count files, F5 to start compression.":
    "Henüz günlük yok. “Tara” ile dosyaları sayabilir, F5 ile sıkıştırmayı başlatabilirsiniz.",
  "Files are not modified in place; all output is written to the destination folder.":
    "Dosyalar yerinde değiştirilmez; tüm çıktılar hedef klasöre yazılır.",
  "Cancel": "İptal Et",
  "Start compression": "Sıkıştırmayı Başlat",
  "Cancels the running job (F5)": "Çalışan işi iptal eder (F5)",
  "Starts compression (F5)": "Sıkıştırmayı başlatır (F5)",
  "Report": "Rapor",

  "No hardware encoder found; using CPU (libx264/libx265).":
    "Donanım kodlayıcı bulunamadı; CPU (libx264/libx265) kullanılacak.",
  "Low / older CPU": "Düşük / eski işlemci",
  "Mid-range": "Orta seviye",
  "Fast": "Hızlı",
  "Very fast": "Çok hızlı",

  // --- Toasts / notifications ---
  "Select a source folder first": "Önce bir kaynak klasör seçin",
  "Scanned {n} files · {size}": "{n} dosya tarandı · {size}",
  "Select source and destination folders": "Kaynak ve hedef klasörü seçin",
  "Started: {msg}": "Başlatıldı: {msg}",
  "No running job": "Çalışan bir iş yok",
  "Complete a job first": "Önce bir işi tamamlayın",
  "Select a destination folder first": "Önce hedef klasörü seçin",
  "{name} profile applied": "{name} profili uygulandı",
  "Report saved: {path}": "Rapor kaydedildi: {path}",
  "ShrinkWise report ({format})": "ShrinkWise raporu ({format})",
  "An ongoing job was detected; tracking progress.":
    "Devam eden bir iş algılandı; ilerleme izleniyor.",
  "Cancellation requested; waiting for running files to finish…":
    "İptal istendi; çalışan dosyaların bitmesi bekleniyor…",
  "Job cancelled": "İş iptal edildi",
  "Compression complete": "Sıkıştırma tamamlandı",
  "ShrinkWise · Job cancelled": "ShrinkWise · İş iptal edildi",
  "ShrinkWise · Compression complete": "ShrinkWise · Sıkıştırma tamamlandı",
  "Job cancelled.": "İş iptal edildi.",
  "{in} → {out} · {saved} saved": "{in} → {out} · {saved} kazanç",
  "CPU score: {score} MB/s · {label}": "CPU puanı: {score} MB/sn · {label}",
  "Recommended encoder: {x}": "Önerilen kodlayıcı: {x}",
  "No hardware encoder found; CPU will be used":
    "Donanım kodlayıcı bulunamadı, CPU kullanılacak",
  "{name} ready": "{name} hazır",

  // --- Presets ---
  "Smart": "Akıllı",
  "Archive": "Arşiv",
  "Web": "Web",
  "Maximum": "Maksimum",
  "Lossless Maximum": "Kayıpsız Maksimum",
  "Lossless Balanced": "Kayıpsız Dengeli",
  "Lossless Fast": "Kayıpsız Hızlı",
  "Custom": "Özel",

  // --- File kinds ---
  "Copied": "Kopyalanan",
  "GIF/Animation": "GIF/Animasyon",
  "Text/Code": "Metin/Kod",
  "Text": "Metin",
  "Lossless": "Kayıpsız",
  "Office": "Ofis",

  // --- Plan / duration ---
  "Image quality {q} · Video CRF {crf} · Audio {audio}k · GIF {fps} fps / {w} px · PDF {dpi} dpi":
    "Görsel kalite {q} · Video CRF {crf} · Ses {audio}k · GIF {fps} fps / {w} px · PDF {dpi} dpi",
  "{s} s": "{s} sn",
  "{m} min {s} s": "{m} dk {s} sn",
  "{h} h {m} min": "{h} sa {m} dk",

  // --- Extra UI strings ---
  "When on, only lossless (lossy=0) presets are shown; when off, normal presets":
    "Açıkken yalnızca kayıpsız (lossy=0) presetler görünür; kapalıyken normal presetler",
  "Produce smaller JPEG when there is no transparency.":
    "Saydamlık yoksa daha küçük JPEG üret.",
  "Check that every output can be opened; report failures.":
    "Her çıktının açılabildiğini kontrol et; hatalıları bildir.",
  "Video codec": "Video kodeği",
  "Everything comes with you when you copy the folder.":
    "Klasörü kopyaladığınızda her şey sizinle gelir.",
  "Appearance, work mode and safety preferences.":
    "Görünüm, çalışma biçimi ve güvenlik tercihleri.",
  "Appearance": "Görünüm",
  "Theme and interface scale apply instantly.":
    "Tema ve arayüz ölçeği anında uygulanır.",
  "Hover over a setting to see what it does — its description appears here.":
    "Bir ayarın ne işe yaradığını merak ettiniz mi? Fareyi üzerine getirin — açıklaması burada görünür.",
  "For tighter archive packing": "Arşivlerde daha sıkı paketleme için",
  "Connecting to server.": "Sunucuya bağlanılıyor.",
  "Connecting to server…": "Sunucuya bağlanılıyor…",
  "Download complete": "İndirme tamamlandı",
  "Verifying file integrity.": "Dosya bütünlüğü kontrol ediliyor.",
  "Verifying file integrity…": "Dosya bütünlüğü kontrol ediliyor…",
  "Verifying archive…": "Arşiv doğrulanıyor…",
  "Extracting archive.": "Arşiv açılıyor.",
  "Extracting archive…": "Arşiv açılıyor…",
  "Verifying installation.": "Kurulum doğrulanıyor.",
  "Verifying installation…": "Kurulum doğrulanıyor…",
  "7-Zip ready": "7-Zip hazır",

  "Documents and archives": "Belgeler ve arşivler",
  "PDF recompression needs Ghostscript or qpdf. Office files and ZIP-based archives are rebuilt with maximum deflate.": "PDF sıkıştırma için Ghostscript veya qpdf gerekir. Office dosyaları ve ZIP tabanlı arşivler en yüksek deflate ile yeniden oluşturulur.",
  "PDF mode": "PDF modu",
  "Maximum compression": "En yüksek sıkıştırma",
  "High quality": "Yüksek kalite",
  "Text / vectors": "Metin / vektör",
  "PDF image DPI (0 = auto)": "PDF görüntü DPI (0 = otomatik)",
  "Deep archive processing": "Derin arşiv işleme",
  "Recompress ZIP entries and nested archives. Uses more CPU.": "ZIP girdilerini ve iç içe arşivleri yeniden sıkıştırır. Daha fazla işlemci kullanır.",
  "Nested archive depth": "İç içe arşiv derinliği",
  "This version compresses image, video, audio, GIF, text, SVG, office documents and ZIP-based archives. The following types are currently copied to the destination unchanged:": "Bu sürüm görüntü, video, ses, GIF, metin, SVG, office belgeleri ve ZIP tabanlı arşivleri sıkıştırır. Aşağıdaki türler şu an hedefe değişmeden kopyalanır:",
  "Non-ZIP archives (RAR, 7z)": "ZIP olmayan arşivler (RAR, 7z)",
  "Target-size and SSIM-based smart mode are not implemented yet; the lossy slider controls quality directly.": "Hedef boyut ve SSIM tabanlı akıllı mod henüz uygulanmadı; kayıplılık kaydırıcısı kaliteyi doğrudan kontrol eder.",
  "Results": "Sonuçlar",
  "{n} files processed": "{n} dosya işlendi",
  "Retry failed ({n})": "Başarısızları yeniden dene ({n})",
  "No files in this filter.": "Bu filtrede dosya yok.",
  "Recent runs": "Son çalışmalar",
  "Tip: you can drag a folder onto the window.": "İpucu: bir klasörü pencereye sürükleyebilirsiniz.",
  "Compressed": "Sıkıştırıldı",
  "Failed": "Başarısız",
  "Skipped": "Atlandı",
  "Cancelled": "İptal edildi",

  // --- redesigned home screen ---
  "Input": "Giriş",
  "Output": "Çıkış",
  "Size": "Boyut",
  "Done": "Bitti",
  "WARN": "UYARI",
  "ERR": "HATA",
  "OK": "TAMAM",
  "JOB": "İŞ",
  "INFO": "BİLGİ",
  "higher quality": "yüksek kalite",
  "smallest file": "en küçük dosya",
  "Automatic plan": "Otomatik plan",
  "Lossless mode": "Kayıpsız mod",
  "Back to compression": "Sıkıştırmaya dön",
  "Upscaling": "Yükseltme",
  "Image Upscaling": "Resim Yükseltme",
  "Video Upscaling": "Video Yükseltme",
  "Drop an image or click to browse": "Resim bırakın veya gözatın",
  "Drop a video or click to browse": "Video bırakın veya gözatın",
  "Scale": "Ölçek",
  "Before": "Önce",
  "After": "Sonra",
  "Upscale": "Yükselt",
  "Processing": "İşleniyor",
  "Target resolution": "Hedef çözünürlük",
  "Enlarge images and videos with AI or high-quality resampling.":
    "Görselleri ve videoları yapay zekâ veya yüksek kaliteli yeniden örneklemeyle büyütün.",
  "Not installed": "Kurulu değil",
  "Manage tools": "Araçları yönet",
  "Reading file…": "Dosya okunuyor…",
  "Drop an image here": "Buraya bir görsel bırakın",
  "PNG, JPEG, WebP, BMP, TIFF, GIF or AVIF — click to browse":
    "PNG, JPEG, WebP, BMP, TIFF, GIF veya AVIF — göz atmak için tıklayın",
  "Change file": "Dosyayı değiştir",
  "Upscaling engine": "Yükseltme motoru",
  "Auto": "Otomatik",
  "Uses Real-ESRGAN": "Real-ESRGAN kullanır",
  "Uses Lanczos": "Lanczos kullanır",
  "AI · photos and anime": "YZ · fotoğraf ve anime",
  "AI · anime, 2x only": "YZ · anime, yalnızca 2x",
  "Lanczos": "Lanczos",
  "Built-in · fast": "Dahili · hızlı",
  "Model": "Model",
  "Photo": "Fotoğraf",
  "Anime": "Anime",
  "Working…": "Çalışıyor…",
  "Upscale image": "Görseli yükselt",
  "Result": "Sonuç",
  "Elapsed": "Geçen süre",
  "Drag the slider to compare before and after.":
    "Önce ve sonrayı karşılaştırmak için kaydırıcıyı sürükleyin.",
  "Before and after comparison": "Önce ve sonra karşılaştırması",
  "Drop a video here": "Buraya bir video bırakın",
  "MP4, MKV, MOV, AVI or WebM — click to browse":
    "MP4, MKV, MOV, AVI veya WebM — göz atmak için tıklayın",
  "High": "Yüksek",
  "FFmpeg is required for video upscaling. Install it from Tools.":
    "Video yükseltme için FFmpeg gerekir. Araçlar'dan kurun.",
  "Upscale video": "Videoyu yükselt",
  "upscale": "büyütme",
  "downscale": "küçültme",
  "same size": "aynı boyut",
  "Starting encoder…": "Kodlayıcı başlatılıyor…",
  "Reading image…": "Görsel okunuyor…",
  "Running Real-ESRGAN…": "Real-ESRGAN çalışıyor…",
  "Running Waifu2x…": "Waifu2x çalışıyor…",
  "Resampling with Lanczos…": "Lanczos ile yeniden örnekleniyor…",
  "Preparing preview…": "Önizleme hazırlanıyor…",
  "Upscale complete": "Yükseltme tamamlandı",
  "Reading video…": "Video okunuyor…",
  "Encoding frames…": "Kareler kodlanıyor…",
  "Finalizing…": "Sonlandırılıyor…",
  "Install FFmpeg, 7-Zip and AI upscaling engines with one click.":
    "FFmpeg, 7-Zip ve yapay zekâ yükseltme motorlarını tek tıkla kurun.",
  "AI engines": "YZ motorları",
  "Install": "Kur",
  "Preview": "Önizleme",
  "Run again": "Yeniden çalıştır",
  "Source size": "Kaynak boyutu",
  "Estimated output": "Tahmini çıktı",
  "Choose an image to start.": "Başlamak için bir görsel seçin.",
  "Choose a video to start.": "Başlamak için bir video seçin.",
  "Choose a file to start": "Başlamak için bir dosya seçin",
  "Choose a file": "Dosya seçin",
  "Drop a file here or browse from disk.": "Dosyayı buraya bırakın veya diskten gözatın.",
  "Pick settings": "Ayarları seçin",
  "Choose an engine and scale, or a target resolution.":
    "Bir motor ve ölçek ya da hedef çözünürlük seçin.",
  "Save the result": "Sonucu kaydedin",
  "The upscaled file is written to the Upscaled folder.":
    "Yükseltilmiş dosya Upscaled klasörüne yazılır.",
  "Runs locally. Originals are never modified.":
    "Yerel olarak çalışır. Orijinaller asla değiştirilmez.",
  "Uses Waifu2x": "Waifu2x kullanır",
  "source": "kaynak",
  "Video engine": "Video motoru",
};

export function t(text: string, lang: string): string {
  if (lang !== "tr") return text;
  return TR[text] ?? text;
}

export function tn(text: string, lang: string, vars: Record<string, string | number>): string {
  let out = t(text, lang);
  for (const [key, value] of Object.entries(vars)) {
    out = out.split(`{${key}}`).join(String(value));
  }
  return out;
}

/**
 * Localizes backend log/progress lines, which arrive as final formatted strings.
 * English passes through unchanged; Turkish applies pattern rules, then the dictionary.
 */
export function localizeLog(text: string, lang: string): string {
  if (lang !== "tr" || !text) return text;
  let m: RegExpMatchArray | null;

  if ((m = text.match(/^Scanning: (.*)$/s))) return `Taranıyor: ${m[1]}`;
  if ((m = text.match(/^Scan error: (.*)$/s))) return `Tarama hatası: ${m[1]}`;
  if (text === "No files to process.") return "İşlenecek dosya bulunamadı.";
  if ((m = text.match(/^Retrying (\d+) selected files\.$/)))
    return `Seçilen ${m[1]} dosya yeniden deneniyor.`;
  if ((m = text.match(/^FFmpeg missing: (.*)$/s))) return `FFmpeg yok: ${m[1]}`;
  if ((m = text.match(/^Downloading: ([\d.,]+) MB$/))) return `İndiriliyor: ${m[1]} MB`;
  if ((m = text.match(/^(\d+) files • (.*) • (\d+) workers • lossy (\d+)%$/)))
    return `${m[1]} dosya • ${m[2]} • ${m[3]} işçi • kayıplılık %${m[4]}`;
  if ((m = text.match(/^(.*): (.*) -> (.*) \(([\d.,]+)% smaller\)\s+\[(.*)\]$/))) {
    const method = m[5]
      .replace(/^JPEG • quality /, "JPEG • kalite ")
      .replace(/^WebP • quality /, "WebP • kalite ")
      .replace(/^WebP • lossless$/, "WebP • kayıpsız")
      .replace(/^Animated WebP • /, "Animasyonlu WebP • ")
      .replace(/^PNG • lossless re-encode$/, "PNG • kayıpsız yeniden kodlama")
      .replace(/^PNG • lossless$/, "PNG • kayıpsız")
      .replace(/ • 2-pass palette$/, " • 2 geçiş palet")
      .replace(/ • safe mode$/, " • güvenli mod")
      .replace(/ • ([\d.,]+)% saved$/, " • %$1 kazanç");
    return `${m[1]}: ${m[2]} -> ${m[3]} (%${m[4]} küçük)  [${method}]`;
  }
  if ((m = text.match(/^(.*): copied(?: \((.*)\))?$/))) {
    const note = (m[2] ?? "")
      .replace(/^no gain$/, "kazanç yok")
      .replace(/^FFmpeg missing$/, "FFmpeg yok");
    return `${m[1]}: kopyalandı${note ? ` (${note})` : ""}`;
  }
  if ((m = text.match(/^(.*): ERROR - (.*)$/s))) return `${m[1]}: HATA - ${m[2]}`;
  if (
    (m = text.match(
      /^(Completed|Cancelled) • (.*) -> (.*) • ([\d.,]+)% saved • ([\d.,]+) s(?: • (\d+) failed)?$/,
    ))
  ) {
    const head = m[1] === "Completed" ? "Tamamlandı" : "İptal edildi";
    return `${head} • ${m[2]} -> ${m[3]} • %${m[4]} kazanç • ${m[5]} sn${m[6] ? ` • ${m[6]} hata` : ""}`;
  }
  if ((m = text.match(/^Not enough free space in the destination \((.*) available\)\. Free some space and try again\.$/)))
    return `Hedefte yeterli boş alan yok (${m[1]} kullanılabilir). Yer açıp tekrar deneyin.`;
  if ((m = text.match(/^Low disk space: (.*) available, source total is (.*)\. Files may fail if the disk fills up\.$/)))
    return `Az disk alanı: ${m[1]} kullanılabilir, kaynak toplamı ${m[2]}. Disk dolarsa bazı dosyalar başarısız olabilir.`;
  return TR[text] ?? text;
}
