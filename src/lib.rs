use std::{io};
use std::io::{Read, Seek, SeekFrom, Write};

#[allow(dead_code)]
struct Header {
    signature: u32,
    flavour: u32,
    length: u32,
    num_tables: u16,
    reserved: u16,
    total_sfnt_size: u32,
    major_version: u16,
    minor_version: u16,
    meta_offset: u32,
    meta_length: u32,
    meta_orig_length: u32,
    priv_offset: u32,
    priv_length: u32,
}

struct TableDirectoryEntry {
    tag: u32,
    offset: u32,
    comp_length: u32,
    orig_length: u32,
    orig_checksum: u32,
    otf_offset: u32,
}

fn read_u32_be<R>(input: &mut R) -> io::Result<u32> where R: Read {
    let mut bf = [0; 4];
    input.read_exact(&mut bf)?;

    Ok(u32::from_be_bytes(bf))
}

fn read_u16_be<R>(input: &mut R) -> io::Result<u16> where R: Read {
    let mut bf = [0; 2];
    input.read_exact(&mut bf)?;

    Ok(u16::from_be_bytes(bf))
}

fn write_u32_be<W>(output: &mut W, num: u32) -> io::Result<()> where W: Write {
    assert_eq!(output.write(&num.to_be_bytes())?, 4);
    Ok(())
}

fn write_u16_be<W>(output: &mut W, num: u16) -> io::Result<()> where W: Write {
    assert_eq!(output.write(&num.to_be_bytes())?, 2);
    Ok(())
}

pub fn woff2otf<I, O>(mut input: &mut I, output: &mut O) -> io::Result<()>
    where I: Read + Seek, O: Write {
    let header = Header {
        signature: read_u32_be(input)?,
        flavour: read_u32_be(input)?,
        length: read_u32_be(input)?,
        num_tables: read_u16_be(input)?,
        reserved: read_u16_be(input)?,
        total_sfnt_size: read_u32_be(input)?,
        major_version: read_u16_be(input)?,
        minor_version: read_u16_be(input)?,
        meta_offset: read_u32_be(input)?,
        meta_length: read_u32_be(input)?,
        meta_orig_length: read_u32_be(input)?,
        priv_offset: read_u32_be(input)?,
        priv_length: read_u32_be(input)?,
    };

    write_u32_be(output, header.flavour)?;
    write_u16_be(output, header.num_tables)?;

    let (entry_selector, search_range) = (0..16)
        .map(
            |n| {
                (n, u16::pow(2, n))
            }
        )
        .filter(
            |x| x.1 <= header.num_tables
        )
        .map(
            |x| (x.0, x.1 * 16)
        )
        .last()
        .unwrap();

    write_u16_be(output, search_range)?;
    write_u16_be(output, entry_selector as u16)?;
    let range_shift = header.num_tables * 16 - search_range;
    write_u16_be(output, range_shift)?;

    let mut offset = 4 + 2 + 2 + 2 + 2; // how many bytes have been written yet

    let mut table_directory_entries = Vec::new();
    for _ in 0..header.num_tables {
        table_directory_entries.push(
            TableDirectoryEntry {
                tag: read_u32_be(input)?,
                offset: read_u32_be(input)?,
                comp_length: read_u32_be(input)?,
                orig_length: read_u32_be(input)?,
                orig_checksum: read_u32_be(input)?,
                otf_offset: 0,
            }
        );

        offset += 16;
    };

    for table_directory_entry in &mut table_directory_entries {
        table_directory_entry.otf_offset = offset;
        offset += table_directory_entry.orig_length;

        write_u32_be(output, table_directory_entry.tag)?;
        write_u32_be(output, table_directory_entry.orig_checksum)?;
        write_u32_be(output, table_directory_entry.otf_offset)?;
        write_u32_be(output, table_directory_entry.orig_length)?;

        if offset % 4 != 0 {
            offset += 4 - (offset % 4);
        }
    }

    for table_directory_entry in table_directory_entries {
        input.seek(SeekFrom::Start(table_directory_entry.offset as u64))?;

        if table_directory_entry.comp_length != table_directory_entry.orig_length {
            let mut rd = flate2::read::ZlibDecoder::new(&mut input)
                .take(table_directory_entry.orig_length as u64);
            io::copy(&mut rd, output)?;
        } else {
            let mut rd = input
                .take(table_directory_entry.orig_length as u64);
            io::copy(&mut rd, output)?;
        };

        input.seek(
            SeekFrom::Start(
                (table_directory_entry.otf_offset + table_directory_entry.comp_length) as u64
            )
        )?;

        let end_offset = table_directory_entry.otf_offset + table_directory_entry.orig_length;
        if end_offset % 4 != 0 {
            output.write_all(
                vec![0_u8; (4 - (end_offset % 4)) as usize].as_slice()
            )?;
        }
    }

    Ok(())
}


#[test]
fn test() {
    let input = include_bytes!("../test_assets/OpenSans-Regular.woff");
    let output = Vec::new();
    let mut cursor_i = io::Cursor::new(input);
    let mut cursor_o = io::Cursor::new(output);

    woff2otf(&mut cursor_i, &mut cursor_o).unwrap();

    let expected = include_bytes!("../test_assets/OpenSans.otf");
    assert_eq!(expected, cursor_o.into_inner().as_slice());
}