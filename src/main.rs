use std::collections::HashSet;
use std::fs;
use std::io::{BufReader, Read, Write};
use std::path::PathBuf;
use caesium::initialize_parameters;
use caesium::jpeg::ChromaSubsampling;
use clap::Parser;
use log::LevelFilter;
use zip::write::FileOptions;

#[derive(Parser)]
#[command(version, about)]
struct Args {
    /// EPUB file to minimize
    in_file: PathBuf,
    /// Output EPUB file
    out_file: PathBuf,
    /// Enables verbose logging
    #[arg(short, long)]
    verbose: bool,
    /// Specifies the quality of JPEG images
    #[arg(default_value_t = 50, value_parser = image_quality_in_range)]
    jpeg_quality: u32,
    /// Minimizes fonts
    #[arg(short, long)]
    fonts: bool,
    /// Compresses images
    #[arg(short, long)]
    images: bool,
    /// Minimizes XHTML files be trimming spaces of each line
    #[arg(short, long)]
    xhtml: bool,
}

fn main() {
    // get command line arguments
    let args = Args::parse();

    // set appropriate logging level
    env_logger::builder()
        .filter_level(match args.verbose {
            true => {LevelFilter::max()}
            false => {LevelFilter::Info}
        })
        .init();

    // open original file and create a new file for output
    let file_in = fs::File::open(args.in_file).expect("failed to open input file");
    let mut archive = zip::ZipArchive::new(BufReader::new(file_in)).unwrap();
    let file_out = fs::File::create(args.out_file).expect("failed to create output file");
    let mut zip = zip::ZipWriter::new(file_out);

    // prepare parameters for compressing images
    let mut cs_params = initialize_parameters();
    cs_params.keep_metadata = true;
    cs_params.jpeg.quality = args.jpeg_quality;
    cs_params.jpeg.chroma_subsampling = ChromaSubsampling::CS411;

    // create list of used characters
    let mut font_files = Vec::new();
    let mut used_chars = HashSet::new();
    for i in 0..255 {
        used_chars.insert(i);
    }

    // iterate over ZIP entries that EPUB uses
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).unwrap();
        if args.images && file.is_file() && file.name().ends_with(".jpg") {
            log::trace!("Compressing image {}", file.name());

            // read the image file
            let mut in_data: Vec<u8> = Vec::new();
            file.read_to_end(&mut in_data).expect("failed to read");

            // compress the image using Caesium
            let result = caesium::compress_in_memory(in_data, &mut cs_params).expect("failed to compress image");

            // write the compressed image to the new EPUB file
            zip.start_file(file.name(), FileOptions::default()).expect("failed to start file");
            zip.write_all(&*result).expect("failed to write compressed");
        } else if args.fonts && file.is_file() && file.name().ends_with(".otf") {
            log::trace!("Detected font file {}", file.name());

            // store the index of the font file for later
            font_files.push(i);
        } else if (args.xhtml || args.fonts) && file.is_file() && file.name().ends_with(".xhtml") {
            log::trace!("Scanning xhtml {}", file.name());

            // read the XHTML file
            let mut in_data: Vec<u8> = Vec::new();
            file.read_to_end(&mut in_data).expect("failed to read file");
            let buffer = String::from_utf8(in_data.clone()).expect("failed to create string from utf-8 file");

            // keep track of used characters
            if args.fonts {
                for x in buffer.encode_utf16() {
                    used_chars.insert(x);
                }
            }

            zip.start_file(file.name(), FileOptions::default()).expect("failed to start file");
            if args.xhtml {
                // trim each line
                let mut new_buffer = String::new();
                for x in buffer.lines() {
                    new_buffer.push_str(x.trim());
                    new_buffer.push_str("\r\n");
                }
                // write the smaller XHTML file
                zip.write(new_buffer.as_bytes()).expect("failed to write compressed");
            } else {
                // write the original file
                zip.write(&*in_data).expect("failed to copy");
            }
        } else {
            // copy any other file we dont use
            zip.raw_copy_file(file).expect("failed to copy file");
        }
    }

    // remove unused glyphs from fonts
    if args.fonts {
        let used_chars = used_chars.drain().collect::<Vec<u16>>();
        for i in font_files {
            let mut file = archive.by_index(i).unwrap();
            log::trace!("Compressing font {}", file.name());

            // read the font file
            let mut in_data: Vec<u8> = Vec::new();
            file.read_to_end(&mut in_data).expect("failed to read");

            // remove unused glyphs from the file
            let font_profile = subsetter::Profile::pdf(&*used_chars);
            let result = subsetter::subset(&*in_data, 0, font_profile).expect("failed to reduce font");

            // write the smaller font file
            zip.start_file(file.name(), FileOptions::default()).expect("failed to start file");
            zip.write_all(&*result).expect("failed to write compressed");
        }
    }

    zip.finish().expect("failed to write");
}

fn image_quality_in_range(s: &str) -> Result<u32, String> {
    let x: usize = s
        .parse()
        .map_err(|_| format!("`{s}` isn't a whole number"))?;
    if (1..100).contains(&x) {
        Ok(x as u32)
    } else {
        Err(format!(
            "image quality not in range 1-100",
        ))
    }
}
