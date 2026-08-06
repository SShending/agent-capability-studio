use caseless::Caseless;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    io::{Read, Seek, SeekFrom, Write},
    path::{Component, Path},
};
use thiserror::Error;
use unicode_normalization::UnicodeNormalization;
use zip::{
    write::SimpleFileOptions, CompressionMethod, DateTime, ZipArchive, ZipWriter,
};

pub const BUNDLE_FORMAT: &str = "agent-skill-studio/skill-bundle";
pub const BUNDLE_FORMAT_VERSION: u32 = 1;
pub const CODEX_CONTRACT_ID: &str = "codex";
pub const CODEX_CONTRACT_VERSION: u32 = 1;
pub const MANIFEST_PATH: &str = "skill-bundle.json";

pub const MAX_ARCHIVE_BYTES: u64 = 256 * 1024 * 1024;
pub const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
pub const MAX_SKILLS: usize = 256;
pub const MAX_FILES_PER_SKILL: usize = 512;
pub const MAX_TOTAL_FILES: usize = 8_192;
pub const MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;
pub const MAX_SKILL_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_TOTAL_BYTES: u64 = 512 * 1024 * 1024;
pub const MAX_PATH_DEPTH: usize = 16;
pub const MAX_COMPONENT_BYTES: usize = 255;
pub const MAX_PATH_BYTES: usize = 1_024;
const MAX_ARCHIVE_PATH_BYTES: usize = MAX_PATH_BYTES + MAX_COMPONENT_BYTES + 8;

const LOCAL_FILE_HEADER: u32 = 0x0403_4b50;
const CENTRAL_DIRECTORY_HEADER: u32 = 0x0201_4b50;
const END_OF_CENTRAL_DIRECTORY: u32 = 0x0605_4b50;
const EOCD_SIZE: u64 = 22;
const CENTRAL_HEADER_SIZE: u64 = 46;
const LOCAL_HEADER_SIZE: u64 = 30;
const UTF8_FLAG: u16 = 1 << 11;

#[derive(Debug, Error)]
pub enum BundleError {
    #[error("The selected file is not a valid Skill Bundle archive.")]
    InvalidArchive,
    #[error("The Skill Bundle uses an unsupported archive feature.")]
    UnsupportedArchiveFeature,
    #[error("The Skill Bundle contains an unsafe archive entry.")]
    UnsafeEntry,
    #[error("The Skill Bundle contains a duplicate archive entry.")]
    DuplicateEntry,
    #[error("The Skill Bundle manifest is missing.")]
    MissingManifest,
    #[error("The Skill Bundle manifest is invalid.")]
    InvalidManifest,
    #[error("This Skill Bundle format version is not supported.")]
    UnsupportedVersion,
    #[error("The Skill Bundle exceeds the accepted resource limits.")]
    LimitExceeded,
    #[error("The Skill Bundle contains a file that is not declared by its manifest.")]
    UnexpectedEntry,
    #[error("The Skill Bundle is missing a file declared by its manifest.")]
    MissingEntry,
    #[error("A bundled file size does not match its manifest evidence.")]
    SizeMismatch,
    #[error("A bundled file hash does not match its manifest evidence.")]
    HashMismatch,
    #[error("A Skill or Bundle revision does not match its manifest evidence.")]
    RevisionMismatch,
    #[error("The Skill Bundle could not be read.")]
    Io(#[from] std::io::Error),
}

impl BundleError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidArchive => "INVALID_BUNDLE",
            Self::UnsupportedArchiveFeature => "UNSUPPORTED_ARCHIVE_FEATURE",
            Self::UnsafeEntry => "UNSAFE_ARCHIVE_ENTRY",
            Self::DuplicateEntry => "DUPLICATE_ARCHIVE_ENTRY",
            Self::MissingManifest => "BUNDLE_MANIFEST_MISSING",
            Self::InvalidManifest => "INVALID_BUNDLE_MANIFEST",
            Self::UnsupportedVersion => "UNSUPPORTED_BUNDLE_VERSION",
            Self::LimitExceeded => "BUNDLE_LIMIT_EXCEEDED",
            Self::UnexpectedEntry => "UNEXPECTED_BUNDLE_FILE",
            Self::MissingEntry => "MISSING_BUNDLE_FILE",
            Self::SizeMismatch => "BUNDLE_SIZE_MISMATCH",
            Self::HashMismatch => "BUNDLE_HASH_MISMATCH",
            Self::RevisionMismatch => "BUNDLE_REVISION_MISMATCH",
            Self::Io(_) => "BUNDLE_IO_ERROR",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BundleManifest {
    pub format: String,
    pub format_version: u32,
    pub agent_contract: AgentContract,
    pub skills: Vec<BundleSkill>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentContract {
    pub id: String,
    pub version: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BundleSkill {
    pub directory_name: String,
    pub revision: String,
    pub files: Vec<BundleFile>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BundleFile {
    pub path: String,
    pub size: u64,
    pub sha256: String,
    pub executable: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BundleInspection {
    pub manifest: BundleManifest,
    pub bundle_revision: String,
    pub total_files: usize,
    pub total_bytes: u64,
}

pub struct BundleFileReader<'a> {
    pub reader: &'a mut dyn Read,
}

#[derive(Clone, Debug)]
struct ArchiveEntry {
    name: String,
    compressed_size: u64,
    uncompressed_size: u64,
    crc32: u32,
    method: u16,
    flags: u16,
    local_offset: u64,
    data_start: u64,
    data_end: u64,
}

pub fn inspect_bundle<R: Read + Seek>(reader: &mut R) -> Result<BundleInspection, BundleError> {
    let archive_length = reader.seek(SeekFrom::End(0))?;
    if archive_length > MAX_ARCHIVE_BYTES {
        return Err(BundleError::LimitExceeded);
    }
    let structural_entries = validate_zip_structure(reader, archive_length)?;
    reader.seek(SeekFrom::Start(0))?;
    let mut archive = ZipArchive::new(reader).map_err(|_| BundleError::InvalidArchive)?;
    if archive.len() != structural_entries.len() {
        return Err(BundleError::InvalidArchive);
    }

    let manifest_entry = structural_entries
        .first()
        .filter(|entry| entry.name == MANIFEST_PATH)
        .ok_or(BundleError::MissingManifest)?;
    if manifest_entry.uncompressed_size > MAX_MANIFEST_BYTES {
        return Err(BundleError::LimitExceeded);
    }
    let mut manifest_bytes = Vec::with_capacity(manifest_entry.uncompressed_size as usize);
    archive
        .by_index(0)
        .map_err(|_| BundleError::InvalidArchive)?
        .take(MAX_MANIFEST_BYTES + 1)
        .read_to_end(&mut manifest_bytes)
        .map_err(|_| BundleError::InvalidArchive)?;
    if manifest_bytes.len() as u64 != manifest_entry.uncompressed_size {
        return Err(BundleError::SizeMismatch);
    }
    let manifest: BundleManifest =
        serde_json::from_slice(&manifest_bytes).map_err(|_| BundleError::InvalidManifest)?;
    validate_manifest(&manifest)?;

    let expected_entries = expected_archive_entries(&manifest);
    if expected_entries.len() != structural_entries.len() {
        return if expected_entries.len() > structural_entries.len() {
            Err(BundleError::MissingEntry)
        } else {
            Err(BundleError::UnexpectedEntry)
        };
    }
    for (actual, expected) in structural_entries.iter().zip(&expected_entries) {
        if actual.name != expected.0 {
            return if expected_entries
                .iter()
                .any(|(path, _)| path == &actual.name)
            {
                Err(BundleError::InvalidArchive)
            } else {
                Err(BundleError::UnexpectedEntry)
            };
        }
    }

    let mut total_bytes = 0_u64;
    for (index, (_, expected)) in expected_entries.iter().enumerate().skip(1) {
        let expected = expected.ok_or(BundleError::InvalidManifest)?;
        let structural = &structural_entries[index];
        if structural.uncompressed_size != expected.size {
            return Err(BundleError::SizeMismatch);
        }
        let mut file = archive
            .by_index(index)
            .map_err(|_| BundleError::InvalidArchive)?;
        let mut digest = Sha256::new();
        let mut bytes_read = 0_u64;
        let mut buffer = [0_u8; 16 * 1024];
        loop {
            let count = file
                .read(&mut buffer)
                .map_err(|_| BundleError::InvalidArchive)?;
            if count == 0 {
                break;
            }
            bytes_read = bytes_read
                .checked_add(count as u64)
                .ok_or(BundleError::LimitExceeded)?;
            if bytes_read > expected.size || bytes_read > MAX_FILE_BYTES {
                return Err(BundleError::LimitExceeded);
            }
            digest.update(&buffer[..count]);
        }
        if bytes_read != expected.size {
            return Err(BundleError::SizeMismatch);
        }
        if format!("{:x}", digest.finalize()) != expected.sha256 {
            return Err(BundleError::HashMismatch);
        }
        total_bytes = total_bytes
            .checked_add(bytes_read)
            .ok_or(BundleError::LimitExceeded)?;
    }

    Ok(BundleInspection {
        bundle_revision: bundle_revision(&manifest)?,
        total_files: expected_entries.len() - 1,
        total_bytes,
        manifest,
    })
}

pub fn write_bundle<W: Write + Seek>(
    writer: W,
    manifest: &BundleManifest,
    readers: &mut [BundleFileReader<'_>],
) -> Result<W, BundleError> {
    validate_manifest(manifest)?;
    writable_bundle_size(manifest)?;
    let expected_entries = expected_archive_entries(manifest);
    let expected_file_count = expected_entries.len() - 1;
    if readers.len() != expected_file_count {
        return if readers.len() < expected_file_count {
            Err(BundleError::MissingEntry)
        } else {
            Err(BundleError::UnexpectedEntry)
        };
    }

    let manifest_bytes = serde_json::to_vec(manifest).map_err(|_| BundleError::InvalidManifest)?;
    if manifest_bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(BundleError::LimitExceeded);
    }
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .last_modified_time(DateTime::default())
        .unix_permissions(0o644);
    let mut archive = ZipWriter::new(writer);
    archive
        .start_file(MANIFEST_PATH, options)
        .map_err(map_zip_write_error)?;
    archive.write_all(&manifest_bytes)?;

    for ((archive_path, expected), source) in expected_entries
        .iter()
        .skip(1)
        .zip(readers.iter_mut())
    {
        let expected = expected.ok_or(BundleError::InvalidManifest)?;
        archive
            .start_file(archive_path, options)
            .map_err(map_zip_write_error)?;
        let mut digest = Sha256::new();
        let mut bytes_read = 0_u64;
        let mut buffer = [0_u8; 16 * 1024];
        loop {
            let count = source.reader.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            bytes_read = bytes_read
                .checked_add(count as u64)
                .ok_or(BundleError::LimitExceeded)?;
            if bytes_read > expected.size || bytes_read > MAX_FILE_BYTES {
                return Err(BundleError::SizeMismatch);
            }
            digest.update(&buffer[..count]);
            archive.write_all(&buffer[..count])?;
        }
        if bytes_read != expected.size {
            return Err(BundleError::SizeMismatch);
        }
        if format!("{:x}", digest.finalize()) != expected.sha256 {
            return Err(BundleError::HashMismatch);
        }
    }
    archive.finish().map_err(map_zip_write_error)
}

pub fn writable_bundle_size(manifest: &BundleManifest) -> Result<u64, BundleError> {
    validate_manifest(manifest)?;
    let manifest_bytes = serde_json::to_vec(manifest).map_err(|_| BundleError::InvalidManifest)?;
    if manifest_bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(BundleError::LimitExceeded);
    }
    let entries = expected_archive_entries(manifest);
    let mut size = EOCD_SIZE;
    for (index, (path, file)) in entries.iter().enumerate() {
        let content_size = if index == 0 {
            manifest_bytes.len() as u64
        } else {
            file.ok_or(BundleError::InvalidManifest)?.size
        };
        let name_size = path.len() as u64;
        size = size
            .checked_add(LOCAL_HEADER_SIZE)
            .and_then(|value| value.checked_add(name_size))
            .and_then(|value| value.checked_add(content_size))
            .and_then(|value| value.checked_add(CENTRAL_HEADER_SIZE))
            .and_then(|value| value.checked_add(name_size))
            .ok_or(BundleError::LimitExceeded)?;
    }
    if size > MAX_ARCHIVE_BYTES {
        return Err(BundleError::LimitExceeded);
    }
    Ok(size)
}

pub fn visit_bundle_files<R, F>(
    reader: &mut R,
    mut visitor: F,
) -> Result<BundleInspection, BundleError>
where
    R: Read + Seek,
    F: FnMut(&BundleSkill, &BundleFile, &mut dyn Read) -> std::io::Result<()>,
{
    let inspection = inspect_bundle(reader)?;
    reader.seek(SeekFrom::Start(0))?;
    let mut archive = ZipArchive::new(reader).map_err(|_| BundleError::InvalidArchive)?;
    let mut index = 1;
    for skill in &inspection.manifest.skills {
        for expected in &skill.files {
            let file = archive
                .by_index(index)
                .map_err(|_| BundleError::InvalidArchive)?;
            let mut verified = VerifyingReader {
                inner: file,
                digest: Sha256::new(),
                bytes_read: 0,
                expected_size: expected.size,
                exceeded_size: false,
                eof_confirmed: false,
            };
            if let Err(error) = visitor(skill, expected, &mut verified) {
                return if verified.exceeded_size {
                    Err(BundleError::SizeMismatch)
                } else {
                    Err(BundleError::Io(error))
                };
            }
            if verified.bytes_read != expected.size {
                return Err(BundleError::SizeMismatch);
            }
            if !verified.confirm_eof()? {
                return Err(BundleError::SizeMismatch);
            }
            if format!("{:x}", verified.digest.finalize()) != expected.sha256 {
                return Err(BundleError::HashMismatch);
            }
            index += 1;
        }
    }
    Ok(inspection)
}

struct VerifyingReader<R> {
    inner: R,
    digest: Sha256,
    bytes_read: u64,
    expected_size: u64,
    exceeded_size: bool,
    eof_confirmed: bool,
}

impl<R: Read> VerifyingReader<R> {
    fn confirm_eof(&mut self) -> std::io::Result<bool> {
        if self.eof_confirmed {
            return Ok(true);
        }
        let mut probe = [0_u8; 1];
        if self.inner.read(&mut probe)? == 0 {
            self.eof_confirmed = true;
            Ok(true)
        } else {
            self.exceeded_size = true;
            Ok(false)
        }
    }
}

impl<R: Read> Read for VerifyingReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        let remaining = self
            .expected_size
            .checked_sub(self.bytes_read)
            .ok_or_else(|| std::io::Error::other("verified Bundle size overflow"))?;
        if remaining == 0 {
            return if self.confirm_eof()? {
                Ok(0)
            } else {
                Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "verified Bundle file exceeds its declared size",
                ))
            };
        }
        let allowed = buffer.len().min(remaining as usize);
        let count = self.inner.read(&mut buffer[..allowed])?;
        self.bytes_read = self
            .bytes_read
            .checked_add(count as u64)
            .ok_or_else(|| std::io::Error::other("verified Bundle size overflow"))?;
        self.digest.update(&buffer[..count]);
        Ok(count)
    }
}

pub fn skill_revision(files: &[BundleFile]) -> Result<String, BundleError> {
    validate_revision_files(files)?;
    let mut digest = Sha256::new();
    digest.update(b"ASS-SKILL\0");
    digest.update(BUNDLE_FORMAT_VERSION.to_be_bytes());
    digest.update(u32_value(files.len())?.to_be_bytes());
    for file in files {
        hash_string(&mut digest, &file.path)?;
        digest.update(file.size.to_be_bytes());
        digest.update(decode_sha256(&file.sha256)?);
        digest.update([u8::from(file.executable)]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

pub fn bundle_revision(manifest: &BundleManifest) -> Result<String, BundleError> {
    validate_manifest(manifest)?;
    let mut digest = Sha256::new();
    digest.update(b"ASS-BUNDLE\0");
    digest.update(manifest.format_version.to_be_bytes());
    hash_string(&mut digest, &manifest.agent_contract.id)?;
    digest.update(manifest.agent_contract.version.to_be_bytes());
    digest.update(u32_value(manifest.skills.len())?.to_be_bytes());
    for skill in &manifest.skills {
        hash_string(&mut digest, &skill.directory_name)?;
        digest.update(decode_sha256(&skill.revision)?);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn validate_manifest(manifest: &BundleManifest) -> Result<(), BundleError> {
    if manifest.format != BUNDLE_FORMAT {
        return Err(BundleError::InvalidManifest);
    }
    if manifest.format_version != BUNDLE_FORMAT_VERSION
        || manifest.agent_contract.id != CODEX_CONTRACT_ID
        || manifest.agent_contract.version != CODEX_CONTRACT_VERSION
    {
        return Err(BundleError::UnsupportedVersion);
    }
    if manifest.skills.is_empty() || manifest.skills.len() > MAX_SKILLS {
        return Err(BundleError::LimitExceeded);
    }

    let mut directory_keys = HashSet::new();
    let mut previous_directory: Option<&str> = None;
    let mut total_files = 0_usize;
    let mut total_bytes = 0_u64;
    for skill in &manifest.skills {
        validate_directory_name(&skill.directory_name)?;
        if previous_directory.is_some_and(|value| value.as_bytes() >= skill.directory_name.as_bytes())
        {
            return Err(BundleError::InvalidManifest);
        }
        previous_directory = Some(&skill.directory_name);
        if !directory_keys.insert(portability_key(&skill.directory_name)) {
            return Err(BundleError::InvalidManifest);
        }
        if skill.files.is_empty() || skill.files.len() > MAX_FILES_PER_SKILL {
            return Err(BundleError::LimitExceeded);
        }
        validate_revision_files(&skill.files)?;
        total_files = total_files
            .checked_add(skill.files.len())
            .ok_or(BundleError::LimitExceeded)?;
        if total_files > MAX_TOTAL_FILES {
            return Err(BundleError::LimitExceeded);
        }

        let mut path_keys = HashSet::new();
        let mut previous_path: Option<&str> = None;
        let mut skill_bytes = 0_u64;
        for file in &skill.files {
            validate_relative_path(&file.path)?;
            if previous_path.is_some_and(|value| value.as_bytes() >= file.path.as_bytes()) {
                return Err(BundleError::InvalidManifest);
            }
            previous_path = Some(&file.path);
            if !path_keys.insert(portability_key(&file.path)) {
                return Err(BundleError::InvalidManifest);
            }
            if file.size > MAX_FILE_BYTES {
                return Err(BundleError::LimitExceeded);
            }
            if !valid_sha256(&file.sha256) {
                return Err(BundleError::InvalidManifest);
            }
            skill_bytes = skill_bytes
                .checked_add(file.size)
                .ok_or(BundleError::LimitExceeded)?;
        }
        if !skill.files.iter().any(|file| file.path == "SKILL.md") {
            return Err(BundleError::InvalidManifest);
        }
        if skill_bytes > MAX_SKILL_BYTES {
            return Err(BundleError::LimitExceeded);
        }
        total_bytes = total_bytes
            .checked_add(skill_bytes)
            .ok_or(BundleError::LimitExceeded)?;
        if total_bytes > MAX_TOTAL_BYTES {
            return Err(BundleError::LimitExceeded);
        }
        if !valid_sha256(&skill.revision) || skill_revision(&skill.files)? != skill.revision {
            return Err(BundleError::RevisionMismatch);
        }
    }
    Ok(())
}

fn expected_archive_entries(
    manifest: &BundleManifest,
) -> Vec<(String, Option<&BundleFile>)> {
    let mut entries = Vec::with_capacity(
        1 + manifest
            .skills
            .iter()
            .map(|skill| skill.files.len())
            .sum::<usize>(),
    );
    entries.push((MANIFEST_PATH.into(), None));
    for skill in &manifest.skills {
        for file in &skill.files {
            entries.push((
                format!("skills/{}/{}", skill.directory_name, file.path),
                Some(file),
            ));
        }
    }
    entries
}

fn validate_directory_name(value: &str) -> Result<(), BundleError> {
    if value.contains(['/', '\\', '\0']) || !valid_portable_component(value)
    {
        Err(BundleError::InvalidManifest)
    } else {
        Ok(())
    }
}

fn validate_relative_path(value: &str) -> Result<(), BundleError> {
    if value.is_empty()
        || value.len() > MAX_PATH_BYTES
        || value.contains(['\\', '\0'])
        || value.chars().any(char::is_control)
    {
        return Err(BundleError::InvalidManifest);
    }
    let path = Path::new(value);
    let components = path.components().collect::<Vec<_>>();
    if components.is_empty()
        || components.len() > MAX_PATH_DEPTH
        || components
            .iter()
            .any(|component| !matches!(component, Component::Normal(_)))
        || value
            .split('/')
            .any(|component| !valid_portable_component(component))
    {
        Err(BundleError::InvalidManifest)
    } else {
        Ok(())
    }
}

fn portability_key(value: &str) -> String {
    value
        .chars()
        .nfd()
        .default_case_fold()
        .nfd()
        .collect()
}

fn validate_revision_files(files: &[BundleFile]) -> Result<(), BundleError> {
    if files.is_empty() {
        return Err(BundleError::InvalidManifest);
    }
    let mut previous: Option<&str> = None;
    let mut keys = HashSet::new();
    for file in files {
        validate_relative_path(&file.path)?;
        if previous.is_some_and(|value| value.as_bytes() >= file.path.as_bytes()) {
            return Err(BundleError::InvalidManifest);
        }
        previous = Some(&file.path);
        if !keys.insert(portability_key(&file.path)) || !valid_sha256(&file.sha256) {
            return Err(BundleError::InvalidManifest);
        }
    }
    Ok(())
}

fn valid_portable_component(value: &str) -> bool {
    if value.is_empty()
        || value.len() > MAX_COMPONENT_BYTES
        || value == "."
        || value == ".."
        || value.ends_with([' ', '.'])
        || value
            .chars()
            .any(|character| character.is_control() || r#"<>:"|?*"#.contains(character))
    {
        return false;
    }
    let base = value.split('.').next().unwrap_or_default().to_ascii_uppercase();
    !matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$")
        && !(base.len() == 4
            && (base.starts_with("COM") || base.starts_with("LPT"))
            && matches!(base.as_bytes()[3], b'1'..=b'9'))
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn decode_sha256(value: &str) -> Result<[u8; 32], BundleError> {
    if !valid_sha256(value) {
        return Err(BundleError::InvalidManifest);
    }
    let mut bytes = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        bytes[index] = (hex_value(pair[0]) << 4) | hex_value(pair[1]);
    }
    Ok(bytes)
}

fn hex_value(value: u8) -> u8 {
    match value {
        b'0'..=b'9' => value - b'0',
        b'a'..=b'f' => value - b'a' + 10,
        _ => unreachable!("validated lowercase hexadecimal"),
    }
}

fn hash_string(digest: &mut Sha256, value: &str) -> Result<(), BundleError> {
    digest.update(u32_value(value.len())?.to_be_bytes());
    digest.update(value.as_bytes());
    Ok(())
}

fn u32_value(value: usize) -> Result<u32, BundleError> {
    value.try_into().map_err(|_| BundleError::LimitExceeded)
}

fn validate_zip_structure<R: Read + Seek>(
    reader: &mut R,
    archive_length: u64,
) -> Result<Vec<ArchiveEntry>, BundleError> {
    if archive_length < EOCD_SIZE {
        return Err(BundleError::InvalidArchive);
    }
    reader.seek(SeekFrom::End(-(EOCD_SIZE as i64)))?;
    let mut eocd = [0_u8; EOCD_SIZE as usize];
    archive_read_exact(reader, &mut eocd)?;
    if le_u32(&eocd, 0)? != END_OF_CENTRAL_DIRECTORY {
        return Err(BundleError::InvalidArchive);
    }
    if le_u16(&eocd, 4)? != 0
        || le_u16(&eocd, 6)? != 0
        || le_u16(&eocd, 8)? != le_u16(&eocd, 10)?
        || le_u16(&eocd, 20)? != 0
    {
        return Err(BundleError::UnsupportedArchiveFeature);
    }
    let entry_count = le_u16(&eocd, 10)? as usize;
    if entry_count == 0 || entry_count > MAX_TOTAL_FILES + 1 {
        return Err(BundleError::LimitExceeded);
    }
    let central_size = le_u32(&eocd, 12)? as u64;
    let central_offset = le_u32(&eocd, 16)? as u64;
    if central_offset
        .checked_add(central_size)
        .and_then(|value| value.checked_add(EOCD_SIZE))
        != Some(archive_length)
    {
        return Err(BundleError::InvalidArchive);
    }

    reader.seek(SeekFrom::Start(central_offset))?;
    let mut entries = Vec::with_capacity(entry_count);
    let mut names = HashSet::new();
    for _ in 0..entry_count {
        let mut header = [0_u8; CENTRAL_HEADER_SIZE as usize];
        archive_read_exact(reader, &mut header)?;
        if le_u32(&header, 0)? != CENTRAL_DIRECTORY_HEADER {
            return Err(BundleError::InvalidArchive);
        }
        let flags = le_u16(&header, 8)?;
        let method = le_u16(&header, 10)?;
        let crc32 = le_u32(&header, 16)?;
        let compressed_size = le_u32(&header, 20)? as u64;
        let uncompressed_size = le_u32(&header, 24)? as u64;
        let name_length = le_u16(&header, 28)? as usize;
        let extra_length = le_u16(&header, 30)? as usize;
        let comment_length = le_u16(&header, 32)? as usize;
        let disk = le_u16(&header, 34)?;
        let external_attributes = le_u32(&header, 38)?;
        let local_offset = le_u32(&header, 42)? as u64;
        if disk != 0
            || extra_length != 0
            || comment_length != 0
            || flags & !UTF8_FLAG != 0
            || !matches!(method, 0 | 8)
            || matches!(compressed_size, 0xffff_ffff)
            || matches!(uncompressed_size, 0xffff_ffff)
            || local_offset == 0xffff_ffff
        {
            return Err(BundleError::UnsupportedArchiveFeature);
        }
        let unix_kind = (external_attributes >> 16) & 0o170000;
        if !matches!(unix_kind, 0 | 0o100000) || external_attributes & 0x10 != 0 {
            return Err(BundleError::UnsafeEntry);
        }
        let central_position = reader.stream_position()?;
        if name_length == 0
            || name_length > MAX_ARCHIVE_PATH_BYTES
            || central_position
                .checked_add(name_length as u64)
                .is_none_or(|end| end > central_offset + central_size)
        {
            return Err(BundleError::InvalidArchive);
        }
        let mut raw_name = vec![0_u8; name_length];
        archive_read_exact(reader, &mut raw_name)?;
        let name = std::str::from_utf8(&raw_name)
            .map_err(|_| BundleError::UnsafeEntry)?
            .to_owned();
        if (!name.is_ascii() && flags & UTF8_FLAG == 0)
            || name.ends_with('/')
            || !valid_archive_path(&name)
        {
            return Err(BundleError::UnsafeEntry);
        }
        if !names.insert(name.clone()) {
            return Err(BundleError::DuplicateEntry);
        }
        entries.push(ArchiveEntry {
            name,
            compressed_size,
            uncompressed_size,
            crc32,
            method,
            flags,
            local_offset,
            data_start: 0,
            data_end: 0,
        });
    }
    if reader.stream_position()? != central_offset + central_size {
        return Err(BundleError::InvalidArchive);
    }

    let mut expected_offset = 0_u64;
    for entry in &mut entries {
        if entry.local_offset != expected_offset {
            return Err(BundleError::InvalidArchive);
        }
        if entry
            .local_offset
            .checked_add(LOCAL_HEADER_SIZE)
            .is_none_or(|end| end > central_offset)
        {
            return Err(BundleError::InvalidArchive);
        }
        reader.seek(SeekFrom::Start(entry.local_offset))?;
        let mut header = [0_u8; LOCAL_HEADER_SIZE as usize];
        archive_read_exact(reader, &mut header)?;
        if le_u32(&header, 0)? != LOCAL_FILE_HEADER
            || le_u16(&header, 6)? != entry.flags
            || le_u16(&header, 8)? != entry.method
            || le_u32(&header, 14)? != entry.crc32
            || le_u32(&header, 18)? as u64 != entry.compressed_size
            || le_u32(&header, 22)? as u64 != entry.uncompressed_size
        {
            return Err(BundleError::InvalidArchive);
        }
        let name_length = le_u16(&header, 26)? as usize;
        let extra_length = le_u16(&header, 28)? as usize;
        if extra_length != 0 {
            return Err(BundleError::UnsupportedArchiveFeature);
        }
        if name_length != entry.name.len() {
            return Err(BundleError::InvalidArchive);
        }
        let mut raw_name = vec![0_u8; name_length];
        archive_read_exact(reader, &mut raw_name)?;
        if raw_name != entry.name.as_bytes() {
            return Err(BundleError::InvalidArchive);
        }
        entry.data_start = entry
            .local_offset
            .checked_add(LOCAL_HEADER_SIZE)
            .and_then(|value| value.checked_add(name_length as u64))
            .ok_or(BundleError::InvalidArchive)?;
        entry.data_end = entry
            .data_start
            .checked_add(entry.compressed_size)
            .ok_or(BundleError::InvalidArchive)?;
        if entry.data_end > central_offset {
            return Err(BundleError::InvalidArchive);
        }
        expected_offset = entry.data_end;
    }
    if expected_offset != central_offset {
        return Err(BundleError::InvalidArchive);
    }

    if !matches!(entries.first(), Some(entry) if entry.name == MANIFEST_PATH) {
        return if entries.iter().any(|entry| entry.name == MANIFEST_PATH) {
            Err(BundleError::InvalidArchive)
        } else {
            Err(BundleError::MissingManifest)
        };
    }
    Ok(entries)
}

fn valid_archive_path(value: &str) -> bool {
    if value == MANIFEST_PATH {
        return true;
    }
    if !value.starts_with("skills/") {
        return false;
    }
    if value.len() > MAX_PATH_BYTES + MAX_COMPONENT_BYTES + "skills//".len()
        || value.contains(['\\', '\0'])
        || value.chars().any(char::is_control)
    {
        return false;
    }
    let components = Path::new(value).components().collect::<Vec<_>>();
    !components.is_empty()
        && components.len() <= MAX_PATH_DEPTH + 2
        && components
            .iter()
            .all(|component| matches!(component, Component::Normal(_)))
        && value
            .split('/')
            .all(|component| !component.is_empty() && component.len() <= MAX_COMPONENT_BYTES)
}

fn le_u16(bytes: &[u8], offset: usize) -> Result<u16, BundleError> {
    let value = bytes
        .get(offset..offset + 2)
        .ok_or(BundleError::InvalidArchive)?;
    Ok(u16::from_le_bytes([value[0], value[1]]))
}

fn le_u32(bytes: &[u8], offset: usize) -> Result<u32, BundleError> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or(BundleError::InvalidArchive)?;
    Ok(u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
}

fn archive_read_exact<R: Read>(reader: &mut R, buffer: &mut [u8]) -> Result<(), BundleError> {
    reader.read_exact(buffer).map_err(|error| {
        if error.kind() == std::io::ErrorKind::UnexpectedEof {
            BundleError::InvalidArchive
        } else {
            BundleError::Io(error)
        }
    })
}

fn map_zip_write_error(error: zip::result::ZipError) -> BundleError {
    match error {
        zip::result::ZipError::Io(error) => BundleError::Io(error),
        _ => BundleError::InvalidArchive,
    }
}

#[cfg(test)]
mod tests;
