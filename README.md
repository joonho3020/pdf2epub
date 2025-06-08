
# PDF to Epub converter


## Installing dependencies

```bash
brew install tesseract leptonica
just install_pdfium
```

## QuickStart

```bash
cargo run --release -- --extract-pagenum
```


## TODO

- [ ] Make this multithreaded
- [ ] Try using PDF structure if provided instead of raw OCR
- [ ] Image extraction
- [ ] Table extraction
- [ ] Chapter by chapter
- [ ] Better recognition of table of contents
