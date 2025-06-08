# PDF to Epub converter

Simple program to read a PDF file, perform OCR on it, and output it in epub format.
Mostly because of my bad experience with online PDF to epub converters...

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
- [ ] Better recognition of references
