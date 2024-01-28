# epubshrink

Epub shrink is a command line utility that helps to reduce size of EPUB files.

It tries to remove unused glyphs in fonts, lowers the quality of images and trims spaces in some files.

## Building

```shell
$ cargo build -r
```

## Usage

```shell
$ epubshrink input_file.epub output_file.epub -i -x 30
```

For detailed usage run:
```shell
$ epubshrink --help
```
