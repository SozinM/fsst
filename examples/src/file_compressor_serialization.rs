#![allow(missing_docs, clippy::use_debug)]

//! This is a command line program that expects an input file as an argument,
//! and trains a symbol table that it then uses to compress the file and write it to the disk
//! along with compressor used. After it reads the file with compressor and decodes it.
//!
//! Example:
//!
//! ```
//! cargo run --release --example file_compressor_with_trained_data -- lineitem.tbl
//! ```
use bincode;
use fsst::Compressor;
use std::io::{BufWriter, Write};
use std::{
    fs::File,
    io::Read,
    path::Path,
};

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let output_path = format!("{}.fsst", args[0].clone());
    let decompressed_path = format!("{}.decompressed", args[0].clone());
    let compressor_path = format!("{}.fsst.compressor", args[0].clone());
    let input_path = Path::new(&args[0]);
    let output_path = Path::new(&output_path);
    let decompressed_path = Path::new(&decompressed_path);
    let compressor_path = Path::new(&compressor_path);

    let mut input_string = String::new();
    {
        let mut input_file = File::open(input_path).unwrap();
        input_file.read_to_string(&mut input_string).unwrap();
    }
    let uncompressed_size = input_string.as_bytes().len();
    let lines: Vec<&[u8]> = input_string.lines().map(|line| line.as_bytes()).collect();

    {
        let start = std::time::Instant::now();
        let compressor = Compressor::train(&lines);

        let duration = std::time::Instant::now().duration_since(start);
        println!("train took {}µs", duration.as_micros());

        let mut compressor_file = File::create(compressor_path).unwrap();
        compressor_file
            .write_all(
                bincode::serialize(&compressor)
                    .expect("Serializing compressor")
                    .as_slice(),
            )
            .expect("Writing compressor");

        let mut compressed_size = 0;

        let mut buffer = Vec::with_capacity(8 * 1024 * 1024);
        let output_file = File::create(output_path).unwrap();
        let mut writer = BufWriter::new(output_file);

        let start = std::time::Instant::now();
        for text in lines {
            unsafe { compressor.compress_into(text, &mut buffer) };
            writer.write(buffer.as_slice()).unwrap();
            compressed_size += buffer.len();
        }
        let duration = std::time::Instant::now().duration_since(start);

        println!("compression took {}µs", duration.as_micros());
        println!(
            "compressed {} -> {} ({}%)",
            uncompressed_size,
            compressed_size,
            100.0 * (compressed_size as f64) / (uncompressed_size as f64)
        );
    }

    let mut compressor_file = File::open(compressor_path).unwrap();
    let mut buf = Vec::new();
    compressor_file.read_to_end(&mut buf).expect("Reading compressor content");
    let compressor: Compressor = bincode::deserialize(&buf).expect("Decoding compressor");
    let decompressor = compressor.decompressor();

    let mut buf = Vec::new();
    {
        let mut output_file = File::open(output_path).expect("Open compressed file");
        output_file.read_to_end(&mut buf).unwrap();
    }

    let decompressed_file = File::create(decompressed_path).unwrap();
    let mut writer= BufWriter::new(decompressed_file);

    let start = std::time::Instant::now();

    writer.write_all(decompressor.decompress(&buf).as_slice()).unwrap();

    let duration = std::time::Instant::now().duration_since(start);
    println!("decompression and writing took {}µs", duration.as_micros());
}
