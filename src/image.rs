use crate::types::ExifError;

pub enum FileType {
    Unknown,
    JPEG,
    TIFF,
}

/// Detect the type of an image contained in a byte buffer
pub fn detect_type(contents: &[u8]) -> FileType {
    if contents.len() < 11 {
        return FileType::Unknown;
    }

    if contents[0] == 0xff && contents[1] == 0xd8 &&
                        contents[2] == 0xff && // contents[3] == 0xe0 &&
                        contents[6] == b'J' && contents[7] == b'F' &&
                        contents[8] == b'I' && contents[9] == b'F' &&
                        contents[10] == 0
    {
        return FileType::JPEG;
    }
    if contents[0] == 0xff && contents[1] == 0xd8 &&
                        contents[2] == 0xff && // contents[3] == 0xe0 &&
                        contents[6] == b'E' && contents[7] == b'x' &&
                        contents[8] == b'i' && contents[9] == b'f' &&
                        contents[10] == 0
    {
        return FileType::JPEG;
    }
    if contents[0] == b'I' && contents[1] == b'I' && contents[2] == 42 && contents[3] == 0 {
        /* TIFF little-endian */
        return FileType::TIFF;
    }
    if contents[0] == b'M' && contents[1] == b'M' && contents[2] == 0 && contents[3] == 42 {
        /* TIFF big-endian */
        return FileType::TIFF;
    }

    FileType::Unknown
}

/// Find the embedded TIFF in a JPEG image (that in turn contains the EXIF data)
pub fn find_embedded_tiff_in_jpeg(contents: &[u8]) -> Result<(usize, usize), ExifError> {
    let mut offset = 2 as usize;

    while offset < contents.len() {
        if contents.len() < (offset + 4) {
            return Err(ExifError::JpegWithoutExif(
                "JPEG truncated in marker header".to_string(),
            ));
        }

        let marker: u16 = u16::from(contents[offset]) * 256 + u16::from(contents[offset + 1]);

        if marker < 0xff00 {
            return Err(ExifError::JpegWithoutExif(format!(
                "Invalid marker {:x}",
                marker
            )));
        }

        offset += 2;
        let size = (contents[offset] as usize) * 256 + (contents[offset + 1] as usize);

        if size < 2 {
            return Err(ExifError::JpegWithoutExif(
                "JPEG marker size must be at least 2 (because of the size word)".to_string(),
            ));
        }
        if contents.len() < (offset + size) {
            return Err(ExifError::JpegWithoutExif(
                "JPEG truncated in marker body".to_string(),
            ));
        }

        if marker == 0xffe1 {
            if size < 8 {
                return Err(ExifError::JpegWithoutExif(
                    "EXIF preamble truncated".to_string(),
                ));
            }

            if contents[offset + 2..offset + 8] != [b'E', b'x', b'i', b'f', 0, 0] {
                return Err(ExifError::JpegWithoutExif(
                    "EXIF preamble unrecognized".to_string(),
                ));
            }

            // The offset and size of the block, excluding size and 'Exif\0\0'.
            return Ok((offset + 8, size - 8));
        }
        if marker == 0xffda {
            // last marker
            return Err(ExifError::JpegWithoutExif(
                "Last mark found and no EXIF".to_string(),
            ));
        }
        offset += size;
    }

    Err(ExifError::JpegWithoutExif(
        "Scan past EOF and no EXIF found".to_string(),
    ))
}

#[cfg(test)]
mod test {
    use glob::glob;

    use crate::*;
    use std::fs;
    use std::path::Path;

    const TEST_IMAGE_DIR: &str = "./tests/img";
    const JPEG_PATTERN: &str = "*.jpg";

    #[cfg(test)]
    fn cmp_serialized_exif_with_original<P: AsRef<Path>>(file: P) -> Result<(), std::io::Error> {
        let parsed_exif1 = parse_file(&file).unwrap();
        let serialized_exif1 = parsed_exif1.serialize();

        // Skip over the Exif header ("Exif\0\0").
        let (entries, le) = parse_tiff(&serialized_exif1[6..], &mut vec![]);

        let parsed_exif2 = ExifData {
            mime: "image/jpeg".to_string(),
            entries: entries.unwrap(),
            le,
        };

        let serialized_exif2 = parsed_exif2.serialize();
        assert_eq!(serialized_exif1, serialized_exif2);
        assert_eq!(parsed_exif1, parsed_exif2);

        Ok(())
    }

    #[test]
    fn test_exif_serialization() -> Result<(), std::io::Error> {
        let jpegs = glob(
            Path::new(TEST_IMAGE_DIR)
                .join(JPEG_PATTERN)
                .to_str()
                .expect("Path is not valid unicode."),
        )
        .expect("Failed to read glob pattern")
        .filter_map(Result::ok)
        .collect::<Vec<_>>();

        for jpeg in jpegs {
            cmp_serialized_exif_with_original(&jpeg)?;
        }

        Ok(())
    }
}
