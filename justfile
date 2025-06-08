detection_model   := "https://ocrs-models.s3-accelerate.amazonaws.com/text-detection.rten"
recognition_model := "https://ocrs-models.s3-accelerate.amazonaws.com/text-recognition.rten"

install_pdfium:
  wget https://github.com/bblanchon/pdfium-binaries/releases/download/chromium%2F7215/pdfium-mac-arm64.tgz
  mkdir pdfium
  tar -xvzf pdfium-mac-arm64.tgz -C pdfium


download_ocr_models:
  curl {{detection_model}}   -o models/text-detection.rten
  curl {{recognition_model}} -o models/text-recognition.rten
