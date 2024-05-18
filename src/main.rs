use rayon::prelude::*;
use std::fs::OpenOptions;
use std::io::{self, BufWriter, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
enum GenerationMode {
    Grayscale,
    Colorful,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
struct XorShift32 {
    x: u32,
}

impl XorShift32 {
    const fn new(x: u32) -> Self {
        Self { x }
    }

    fn next(&mut self) -> u32 {
        let mut x = self.x;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.x = x;
        x
    }

    fn step_forward(mut self, steps: usize) -> Self {
        for _ in 0..steps {
            self.next();
        }

        self
    }
}

fn main() -> io::Result<()> {
    let genmode = ask_enum(
        "Enter mode",
        "[ERR] Invalid mode\nValid modes are:\n\t- grayscale\n\t- colorful",
        &[
            ("grayscale", GenerationMode::Grayscale),
            ("colorful", GenerationMode::Colorful),
        ],
        io::stdout().lock(),
    )?;

    let width = ask("Enter width", "[ERR] Invalid width", io::stdout().lock())?;
    let height = ask("Enter height", "[ERR] Invalid height", io::stdout().lock())?;
    let seed: u32 = ask("Enter seed", "[ERR] Invalid seed", io::stdout().lock())?;
    let path: String = ask(
        "Enter output path",
        "[ERR] Invalid path",
        io::stdout().lock(),
    )?;

    let output_file = PathBuf::from(path).with_extension("png");

    let (result, total_time) = time(|| -> io::Result<()> {
        let (rows, generation_time) = time(|| generate_random_pixels(seed, width, height, genmode));
        eprintln!(
            "Generation finished in {}",
            format_duration(generation_time)
        );

        let (img, conversion_time) = time(|| convert_pixels_to_image_buffer(rows, width, height));
        eprintln!(
            "Conversion finished in {}",
            format_duration(conversion_time)
        );

        let (result, write_time) = time(|| write_image_to_file(&output_file, &img));
        result?;
        eprintln!(
            "Written to {:?} in {}",
            output_file,
            format_duration(write_time)
        );

        Ok(())
    });
    result?;
    eprintln!("Total time: {}", format_duration(total_time));

    Ok(())
}

fn pixel_grayscale(num: u32) -> image::Rgb<u8> {
    image::Rgb([num as u8, num as u8, num as u8])
}

fn pixel_colorful(num: u32) -> image::Rgb<u8> {
    let r = num << 24 >> 24;
    let g = num << 16 >> 24;
    let b = num << 8 >> 24;

    image::Rgb([r as u8, g as u8, b as u8])
}

// fn random_pixel(rng: &mut XorShift32, mode: GenerationMode) -> image::Rgb<u8> {
//     match mode {
//         GenerationMode::Grayscale => {
//             let value = rng.next() % 256;
//             image::Rgb([value as u8, value as u8, value as u8])
//         }
//         GenerationMode::Colorful => {
//             let num = rng.next();
//             let r = num << 24 >> 24;
//             let g = num << 16 >> 24;
//             let b = num << 8 >> 24;

//             image::Rgb([r as u8, g as u8, b as u8])
//         }
//     }
// }

fn ask<T, W>(question: &str, error_message: &str, stdout: W) -> io::Result<T>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Debug,
    W: Write,
{
    fn read_line<T, W>(
        question: &str,
        error_message: &str,
        mut stdout: W,
        buffer: &mut String,
    ) -> io::Result<T>
    where
        T: std::str::FromStr,
        T::Err: std::fmt::Debug,
        W: Write,
    {
        write!(stdout, "{}: ", question)?;
        stdout.flush()?;

        io::stdin().read_line(buffer)?;

        match buffer.trim().parse() {
            Ok(value) => Ok(value),
            Err(_) => {
                writeln!(stdout, "{}", error_message)?;
                read_line(question, error_message, stdout, buffer)
            }
        }
    }

    let mut input = String::new();
    read_line(question, error_message, stdout, &mut input)
}

fn ask_enum<T, W>(
    question: &str,
    error_message: &str,
    enum_string_mappings: &[(&str, T)],
    stdout: W,
) -> io::Result<T>
where
    T: Copy,
    W: Write,
{
    fn read_line_enum<T, W>(
        question: &str,
        error_message: &str,
        mut stdout: W,
        enum_string_mappings: &[(&str, T)],
        buffer: &mut String,
    ) -> io::Result<T>
    where
        T: Copy,
        W: Write,
    {
        write!(stdout, "{}: ", question)?;
        stdout.flush()?;

        io::stdin().read_line(buffer)?;

        let checkfor = buffer.trim();

        match enum_string_mappings
            .iter()
            .find(|(string, _)| *string == checkfor)
        {
            Some((_, value)) => Ok(*value),
            None => {
                writeln!(stdout, "{}", error_message)?;
                read_line_enum(
                    question,
                    error_message,
                    stdout,
                    enum_string_mappings,
                    buffer,
                )
            }
        }
    }

    let mut input = String::new();
    read_line_enum(
        question,
        error_message,
        stdout,
        enum_string_mappings,
        &mut input,
    )
}

fn time<T>(f: impl FnOnce() -> T) -> (T, Duration) {
    let timer = Instant::now();
    let result = f();
    let elapsed = timer.elapsed();

    (result, elapsed)
}

fn generate_random_pixels(
    seed: u32,
    width: u32,
    height: u32,
    genmode: GenerationMode,
) -> Vec<image::Rgb<u8>> {
    // // older implementation that worked
    // let mut master_rng =
    //     XorShift32::new(seed.wrapping_mul(0xDEADBEEF).wrapping_add(0xCAFEBABE)).step_forward(100);
    // (0..height)
    //     .map(|_| {
    //         XorShift32::new(
    //             master_rng
    //                 .next()
    //                 .wrapping_mul(0xDEADBEEF)
    //                 .wrapping_add(0xCAFEBABE),
    //         )
    //         .step_forward(100)
    //     })
    //     .collect::<Vec<_>>()
    //     .into_par_iter()
    //     .flat_map(|mut thread_rng| {
    //         let mut row = Vec::with_capacity(width as usize);

    //         for _ in 0..width {
    //             row.push(random_pixel(&mut thread_rng, genmode));
    //         }

    //         row
    //     })
    //     .collect::<Vec<_>>()

    // -------------------------------------------------------------------------------

    // old working example 2
    let mut master_rng =
        XorShift32::new(seed.wrapping_mul(0xDEADBEEF).wrapping_add(0xCAFEBABE)).step_forward(100);

    let rngs = (0..height)
        .map(|_| {
            XorShift32::new(
                master_rng
                    .next()
                    .wrapping_mul(0x4d0df4c7)
                    .wrapping_add(0x8980ab2b),
            )
            .step_forward(100)
        })
        .collect::<Vec<_>>();

    let mut rows = vec![image::Rgb([0, 0, 0]); (width * height) as usize];
    match genmode {
        GenerationMode::Grayscale => {
            rows.par_chunks_exact_mut(width as usize)
                .zip(rngs)
                .for_each(|(row, mut rng)| {
                    for pixel in row {
                        let num = rng.next();
                        *pixel = pixel_grayscale(num);
                    }
                });
        }
        GenerationMode::Colorful => {
            rows.par_chunks_exact_mut(width as usize)
                .zip(rngs)
                .for_each(|(row, mut rng)| {
                    for pixel in row {
                        let num = rng.next();
                        *pixel = pixel_colorful(num);
                    }
                });
        }
    }

    rows

    // let mut rows = vec![image::Rgb([0, 0, 0]); (width * height) as usize];

    // // const F1: u32 = 0x4d0df4c7;
    // // const F2: u32 = 0x8980ab2b;

    // match genmode {
    //     GenerationMode::Grayscale => {
    //         rows.par_chunks_exact_mut(width as usize)
    //             .enumerate()
    //             .for_each(|(y, row)| {
    //                 for (x, pixel) in row.iter_mut().enumerate() {
    //                     let num = superfast_hash(x as u64, y as u64);
    //                     *pixel = pixel_grayscale(num as u32);
    //                 }
    //             });
    //     }
    //     GenerationMode::Colorful => {
    //         rows.par_chunks_exact_mut(width as usize)
    //             .enumerate()
    //             .for_each(|(y, row)| {
    //                 for (x, pixel) in row.iter_mut().enumerate() {
    //                     let num = superfast_hash(x as u64, y as u64);
    //                     *pixel = pixel_colorful(num as u32);
    //                 }
    //             });
    //     }
    // }

    // rows
}

fn convert_pixels_to_image_buffer(
    rows: Vec<image::Rgb<u8>>,
    width: u32,
    height: u32,
) -> image::ImageBuffer<image::Rgb<u8>, std::vec::Vec<u8>> {
    let raw_pixels = rows
        .into_iter()
        .flat_map(|pixel| pixel.0)
        .collect::<Vec<u8>>();

    image::ImageBuffer::from_raw(width, height, raw_pixels)
        .expect("Could not convert generated image into a buffer")
}

fn write_image_to_file(
    output_file: &PathBuf,
    img: &image::ImageBuffer<image::Rgb<u8>, std::vec::Vec<u8>>,
) -> io::Result<()> {
    let bw = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(output_file)?;
    let mut bw = BufWriter::new(bw);

    if let Err(e) = img.write_to(&mut bw, image::ImageFormat::Png) {
        eprintln!("Error writing image: {}", e);
    };

    Ok(())
}

fn format_duration(duration: Duration) -> String {
    if duration == Duration::ZERO {
        "no time (0)".to_string()
    } else if duration < Duration::from_micros(1) {
        format!(
            "{} {}",
            duration.as_nanos(),
            if duration.as_nanos() == 1 {
                "nanosecond"
            } else {
                "nanoseconds"
            }
        )
    } else if duration < Duration::from_millis(5) {
        format!(
            "{} {}",
            duration.as_micros(),
            if duration.as_micros() == 1 {
                "microsecond"
            } else {
                "microseconds"
            }
        )
    } else if duration < Duration::from_secs(1) {
        format!(
            "{} {}",
            duration.as_millis(),
            if duration.as_millis() == 1 {
                "millisecond"
            } else {
                "milliseconds"
            }
        )
    } else if duration > Duration::from_secs(60) {
        let min = duration.as_secs() / 60;
        let sec = duration.as_secs() % 60;
        format!(
            "{} {} {} {}",
            min,
            if min == 1 { "minute" } else { "minutes" },
            sec,
            if sec == 1 { "second" } else { "seconds" }
        )
    } else {
        let mut s = format!(
            "{} {}",
            duration.as_secs(),
            if duration.as_secs() == 1 {
                "second"
            } else {
                "seconds"
            }
        );

        if duration.subsec_nanos() != 0 {
            s.push(' ');
            s.push_str(&format_duration(Duration::from_nanos(
                duration.subsec_nanos() as u64 % 1_000_000_000,
            )));
        }

        s
    }
}
// taken from https://www.shadertoy.com/view/XlGcRh
// fn superfast_hash(x: u64, y: u64) -> u64 {
//     {
//         // uint hash = 8u, tmp;
//         let mut hash: u64 = 8;

//         // hash += data.x & 0xffffu;
//         hash = hash.wrapping_add(x & 0xffff);
//         // tmp = (((data.x >> 16) & 0xffffu) << 11) ^ hash;
//         let mut tmp = (((x >> 16) & 0xffff) << 11) ^ hash;
//         // hash = (hash << 16) ^ tmp;
//         hash = (hash << 16) ^ tmp;

//         // hash += hash >> 11;
//         hash = hash.wrapping_add(hash >> 11);

//         // hash += data.y & 0xffffu;
//         hash = hash.wrapping_add(y & 0xffff);
//         // tmp = (((data.y >> 16) & 0xffffu) << 11) ^ hash;
//         tmp = (((y >> 16) & 0xffff) << 11) ^ hash;

//         // hash = (hash << 16) ^ tmp;
//         hash = (hash << 16) ^ tmp;

//         // hash += hash >> 11;
//         hash = hash.wrapping_add(hash >> 11);

//         // /* Force "avalanching" of final 127 bits */
//         // hash ^= hash << 3;
//         hash ^= hash << 3;
//         // hash += hash >> 5;
//         hash = hash.wrapping_add(hash >> 5);
//         // hash ^= hash << 4;
//         hash ^= hash << 4;
//         // hash += hash >> 17;
//         hash = hash.wrapping_add(hash >> 17);
//         // hash ^= hash << 25;
//         hash ^= hash << 25;
//         // hash += hash >> 6;
//         hash = hash.wrapping_add(hash >> 6);

//         // return hash;
//         hash
//     }
// }
