use super::*;
use std::io::{Cursor, Read, Seek, SeekFrom, Write};
use zip::{
    write::{FullFileOptions, SimpleFileOptions},
    CompressionMethod, ZipWriter,
};

fn file(path: &str, bytes: &[u8], executable: bool) -> BundleFile {
    BundleFile {
        path: path.into(),
        size: bytes.len() as u64,
        sha256: format!("{:x}", Sha256::digest(bytes)),
        executable,
    }
}

fn manifest(files: Vec<BundleFile>) -> BundleManifest {
    let revision = skill_revision(&files).unwrap();
    BundleManifest {
        format: BUNDLE_FORMAT.into(),
        format_version: BUNDLE_FORMAT_VERSION,
        agent_contract: AgentContract {
            id: CODEX_CONTRACT_ID.into(),
            version: CODEX_CONTRACT_VERSION,
        },
        skills: vec![BundleSkill {
            directory_name: "demo".into(),
            revision,
            files,
        }],
    }
}

fn manifest_with_untrusted_files(files: Vec<BundleFile>) -> BundleManifest {
    let mut value = manifest(vec![file("SKILL.md", b"placeholder", false)]);
    value.skills[0].files = files;
    value.skills[0].revision = "f".repeat(64);
    value
}

fn bundle(manifest: &BundleManifest, contents: &[(&str, &[u8], bool)]) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    writer.start_file(MANIFEST_PATH, options).unwrap();
    writer
        .write_all(&serde_json::to_vec(manifest).unwrap())
        .unwrap();
    for (path, bytes, executable) in contents {
        let options = options.unix_permissions(if *executable { 0o755 } else { 0o644 });
        writer.start_file(format!("skills/demo/{path}"), options).unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

#[test]
fn accepts_minimal_bundle_and_computes_stable_revisions() {
    let skill = b"---\nname: demo\ndescription: Use when testing bundles.\n---\n\n# Demo\n";
    let manifest = manifest(vec![file("SKILL.md", skill, false)]);
    let bytes = bundle(&manifest, &[("SKILL.md", skill, false)]);
    let inspection = inspect_bundle(&mut Cursor::new(bytes)).unwrap();
    assert_eq!(
        manifest.skills[0].revision,
        "2ef683119ae5a0e7fabd469e2eca4337293fbb9e7d5aec706637463a834d5d3a"
    );
    assert_eq!(
        inspection.bundle_revision,
        "c10b1f74bf19830b9148111b01c8bc893e08dee94ec2252a1ee551eb07b9e6b4"
    );
    assert_eq!(inspection.manifest, manifest);
    assert_eq!(inspection.total_files, 1);
    assert_eq!(inspection.total_bytes, skill.len() as u64);
    assert_eq!(inspection.bundle_revision, bundle_revision(&manifest).unwrap());
}

#[test]
fn canonical_writer_is_deterministic_and_self_verifying() {
    let skill = b"---\nname: demo\ndescription: Use when exporting.\n---\n";
    let helper = b"#!/bin/sh\necho helper\n";
    let files = vec![
        file("SKILL.md", skill, false),
        file("scripts/helper.sh", helper, true),
    ];
    let manifest = manifest(files);

    let write = || {
        let mut skill_reader = Cursor::new(skill.as_slice());
        let mut helper_reader = Cursor::new(helper.as_slice());
        let mut readers = [
            BundleFileReader {
                reader: &mut skill_reader,
            },
            BundleFileReader {
                reader: &mut helper_reader,
            },
        ];
        write_bundle(Cursor::new(Vec::new()), &manifest, &mut readers)
            .unwrap()
            .into_inner()
    };
    let first = write();
    let second = write();
    assert_eq!(first, second);
    assert_eq!(
        format!("{:x}", Sha256::digest(&first)),
        "38e54715b2b23a92556f0a9da43269b28c0488ad087710aa2be22b3863eaad55"
    );
    let inspection = inspect_bundle(&mut Cursor::new(first)).unwrap();
    assert_eq!(inspection.manifest, manifest);
    assert!(inspection.manifest.skills[0].files[1].executable);
}

#[test]
fn canonical_writer_rejects_reader_count_and_content_drift() {
    let skill = b"skill";
    let manifest = manifest(vec![file("SKILL.md", skill, false)]);
    assert!(matches!(
        write_bundle(Cursor::new(Vec::new()), &manifest, &mut []),
        Err(BundleError::MissingEntry)
    ));

    let mut first = Cursor::new(skill.as_slice());
    let mut second = Cursor::new(skill.as_slice());
    let mut extra = [
        BundleFileReader { reader: &mut first },
        BundleFileReader {
            reader: &mut second,
        },
    ];
    assert!(matches!(
        write_bundle(Cursor::new(Vec::new()), &manifest, &mut extra),
        Err(BundleError::UnexpectedEntry)
    ));

    let mut changed = Cursor::new(b"other".as_slice());
    let mut changed_reader = [BundleFileReader {
        reader: &mut changed,
    }];
    assert!(matches!(
        write_bundle(Cursor::new(Vec::new()), &manifest, &mut changed_reader),
        Err(BundleError::HashMismatch)
    ));
}

#[test]
fn canonical_writer_rejects_manifests_that_cannot_fit_the_archive_limit() {
    let mut manifest = manifest_with_skill_shapes(16, 1, 16 * 1024 * 1024);
    for skill in &mut manifest.skills {
        skill.revision = skill_revision(&skill.files).unwrap();
    }
    assert!(matches!(
        writable_bundle_size(&manifest),
        Err(BundleError::LimitExceeded)
    ));
}

#[test]
fn verified_visitor_exposes_only_manifest_files_in_canonical_order() {
    let skill = b"skill";
    let helper = b"helper";
    let manifest = manifest(vec![
        file("SKILL.md", skill, false),
        file("scripts/helper.txt", helper, false),
    ]);
    let bytes = bundle(
        &manifest,
        &[
            ("SKILL.md", skill, false),
            ("scripts/helper.txt", helper, false),
        ],
    );
    let mut visited = Vec::new();
    let inspection = visit_bundle_files(&mut Cursor::new(bytes), |skill, file, reader| {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes)?;
        visited.push((skill.directory_name.clone(), file.path.clone(), bytes));
        Ok(())
    })
    .unwrap();
    assert_eq!(inspection.manifest, manifest);
    assert_eq!(
        visited,
        vec![
            ("demo".into(), "SKILL.md".into(), skill.to_vec()),
            (
                "demo".into(),
                "scripts/helper.txt".into(),
                helper.to_vec()
            ),
        ]
    );
}

#[test]
fn verified_visitor_requires_each_callback_to_consume_the_declared_file() {
    let skill = b"skill";
    let manifest = manifest(vec![file("SKILL.md", skill, false)]);
    let bytes = bundle(&manifest, &[("SKILL.md", skill, false)]);
    assert!(matches!(
        visit_bundle_files(&mut Cursor::new(bytes), |_skill, _file, _reader| Ok(())),
        Err(BundleError::SizeMismatch)
    ));
}

#[test]
fn second_pass_reader_never_exposes_bytes_past_the_manifest_size() {
    let mut reader = VerifyingReader {
        inner: Cursor::new(b"declared-extra".as_slice()),
        digest: Sha256::new(),
        bytes_read: 0,
        expected_size: b"declared".len() as u64,
        exceeded_size: false,
        eof_confirmed: false,
    };
    let mut consumed = Vec::new();
    let error = reader.read_to_end(&mut consumed).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    assert_eq!(consumed, b"declared");
    assert!(reader.exceeded_size);
}

#[test]
fn executable_mode_changes_skill_and_bundle_revisions() {
    let body = b"#!/bin/sh\necho inspect\n";
    let regular = manifest(vec![
        file("SKILL.md", b"skill", false),
        file("scripts/check.sh", body, false),
    ]);
    let executable = manifest(vec![
        file("SKILL.md", b"skill", false),
        file("scripts/check.sh", body, true),
    ]);
    assert_ne!(regular.skills[0].revision, executable.skills[0].revision);
    assert_ne!(
        bundle_revision(&regular).unwrap(),
        bundle_revision(&executable).unwrap()
    );
    assert_eq!(
        executable.skills[0].revision,
        "b86f636aee9ab230fac82c563a83eeeaac13a12db089ed86d5a6ca9920ac87ad"
    );
    assert_eq!(
        bundle_revision(&executable).unwrap(),
        "42a95ec24aafefaa06d1392e9fa46383e1185fb53b78ec0bc6fe6c274765a63a"
    );

    let bytes = bundle(
        &executable,
        &[
            ("SKILL.md", b"skill", false),
            ("scripts/check.sh", body, false),
        ],
    );
    let inspection = inspect_bundle(&mut Cursor::new(bytes)).unwrap();
    assert!(inspection.manifest.skills[0].files[1].executable);

    let bytes = bundle(
        &regular,
        &[
            ("SKILL.md", b"skill", false),
            ("scripts/check.sh", body, true),
        ],
    );
    let inspection = inspect_bundle(&mut Cursor::new(bytes)).unwrap();
    assert!(!inspection.manifest.skills[0].files[1].executable);
}

#[test]
fn accepts_multiple_skills_nested_binary_files_and_unicode_names() {
    let alpha_skill = b"---\nname: alpha\ndescription: Use when testing alpha.\n---\n";
    let binary = [0, 1, 2, 0xff];
    let beta_skill = b"---\nname: beta\ndescription: Use when testing beta.\n---\n";
    let alpha_files = vec![
        file("SKILL.md", alpha_skill, false),
        file("assets/data.bin", &binary, false),
    ];
    let beta_files = vec![file("SKILL.md", beta_skill, false)];
    let manifest = BundleManifest {
        format: BUNDLE_FORMAT.into(),
        format_version: BUNDLE_FORMAT_VERSION,
        agent_contract: AgentContract {
            id: CODEX_CONTRACT_ID.into(),
            version: CODEX_CONTRACT_VERSION,
        },
        skills: vec![
            BundleSkill {
                directory_name: "alpha".into(),
                revision: skill_revision(&alpha_files).unwrap(),
                files: alpha_files,
            },
            BundleSkill {
                directory_name: "beta-é".into(),
                revision: skill_revision(&beta_files).unwrap(),
                files: beta_files,
            },
        ],
    };
    let bytes = full_bundle(
        &manifest,
        &[
            ("skills/alpha/SKILL.md", alpha_skill),
            ("skills/alpha/assets/data.bin", &binary),
            ("skills/beta-é/SKILL.md", beta_skill),
        ],
        CompressionMethod::Deflated,
    );
    let inspection = inspect_bundle(&mut Cursor::new(bytes)).unwrap();
    assert_eq!(
        manifest.skills[0].revision,
        "e1a79873447ac034581fc338ed3462e5e5bae0fb225766ca41217986da0a41eb"
    );
    assert_eq!(
        manifest.skills[1].revision,
        "9f18ec6bad0dcf3d6ac01e3634891240f790042c2a5cad2093c476aba3034c9e"
    );
    assert_eq!(
        inspection.bundle_revision,
        "f5d58c91cd34d032333c7d5386028b74ae03b229accd3923751bc2f4f2309a45"
    );
    assert_eq!(inspection.total_files, 3);
    assert_eq!(inspection.total_bytes, 4 + alpha_skill.len() as u64 + beta_skill.len() as u64);
}

#[test]
fn rejects_unknown_fields_duplicate_keys_and_versions() {
    let skill = b"skill";
    let manifest = manifest(vec![file("SKILL.md", skill, false)]);
    let mut unknown = serde_json::to_value(&manifest).unwrap();
    unknown["extra"] = serde_json::json!(true);
    let mut bytes = bundle(&manifest, &[("SKILL.md", skill, false)]);
    replace_manifest(&mut bytes, serde_json::to_vec(&unknown).unwrap());
    assert_code(bytes, "INVALID_BUNDLE_MANIFEST");

    let duplicate = format!(
        "{{\"format\":\"{BUNDLE_FORMAT}\",\"format\":\"{BUNDLE_FORMAT}\",\"formatVersion\":1,\"agentContract\":{{\"id\":\"codex\",\"version\":1}},\"skills\":[]}}"
    );
    let mut bytes = bundle(&manifest, &[("SKILL.md", skill, false)]);
    replace_manifest(&mut bytes, duplicate.into_bytes());
    assert_code(bytes, "INVALID_BUNDLE_MANIFEST");

    let mut version = manifest.clone();
    version.format_version = 2;
    assert_code(
        bundle(&version, &[("SKILL.md", skill, false)]),
        "UNSUPPORTED_BUNDLE_VERSION",
    );
}

#[test]
fn rejects_unsorted_and_portability_colliding_manifest_paths() {
    let skill = b"skill";
    let unsorted = manifest_with_untrusted_files(vec![
        file("z.txt", b"z", false),
        file("SKILL.md", skill, false),
    ]);
    assert_code(
        bundle(
            &unsorted,
            &[("z.txt", b"z", false), ("SKILL.md", skill, false)],
        ),
        "INVALID_BUNDLE_MANIFEST",
    );

    let colliding_files = vec![
        file("A.txt", b"a", false),
        file("SKILL.md", skill, false),
        file("a.txt", b"b", false),
    ];
    let colliding = manifest_with_untrusted_files(colliding_files);
    assert_code(
        bundle(
            &colliding,
            &[
                ("A.txt", b"a", false),
                ("SKILL.md", skill, false),
                ("a.txt", b"b", false),
            ],
        ),
        "INVALID_BUNDLE_MANIFEST",
    );

    let normalized_files = vec![
        file("SKILL.md", skill, false),
        file("e\u{301}.txt", b"a", false),
        file("é.txt", b"b", false),
    ];
    let normalized = manifest_with_untrusted_files(normalized_files);
    assert_code(
        bundle(
            &normalized,
            &[
                ("SKILL.md", skill, false),
                ("e\u{301}.txt", b"a", false),
                ("é.txt", b"b", false),
            ],
        ),
        "INVALID_BUNDLE_MANIFEST",
    );

    let casefold = manifest_with_untrusted_files(vec![
        file("SKILL.md", skill, false),
        file("Σ.txt", b"a", false),
        file("ς.txt", b"b", false),
    ]);
    assert_code(
        bundle(
            &casefold,
            &[
                ("SKILL.md", skill, false),
                ("Σ.txt", b"a", false),
                ("ς.txt", b"b", false),
            ],
        ),
        "INVALID_BUNDLE_MANIFEST",
    );

    assert!(matches!(
        skill_revision(&[
            file("z.txt", b"z", false),
            file("SKILL.md", skill, false)
        ]),
        Err(BundleError::InvalidManifest)
    ));
}

#[test]
fn rejects_missing_unexpected_size_hash_and_revision_mismatches() {
    let skill = b"skill";
    let manifest = manifest(vec![file("SKILL.md", skill, false)]);
    assert_code(bundle(&manifest, &[]), "MISSING_BUNDLE_FILE");
    assert_code(
        bundle(
            &manifest,
            &[("SKILL.md", skill, false), ("extra.txt", b"extra", false)],
        ),
        "UNEXPECTED_BUNDLE_FILE",
    );

    let mut wrong_size = manifest.clone();
    wrong_size.skills[0].files[0].size += 1;
    wrong_size.skills[0].revision = skill_revision(&wrong_size.skills[0].files).unwrap();
    assert_code(
        bundle(&wrong_size, &[("SKILL.md", skill, false)]),
        "BUNDLE_SIZE_MISMATCH",
    );

    assert_code(
        bundle(&manifest, &[("SKILL.md", b"other", false)]),
        "BUNDLE_HASH_MISMATCH",
    );

    let mut wrong_revision = manifest.clone();
    wrong_revision.skills[0].revision = "f".repeat(64);
    assert_code(
        bundle(&wrong_revision, &[("SKILL.md", skill, false)]),
        "BUNDLE_REVISION_MISMATCH",
    );
}

#[test]
fn rejects_traversal_absolute_backslash_and_duplicate_archive_entries() {
    let skill = b"skill";
    let manifest = manifest(vec![file("SKILL.md", skill, false)]);
    for path in ["../outside", "/absolute", "skills\\demo\\SKILL.md"] {
        let bytes = raw_bundle(&manifest, &[(path, skill), ("skills/demo/SKILL.md", skill)]);
        assert_code(bytes, "UNSAFE_ARCHIVE_ENTRY");
    }
    let bytes = raw_bundle(
        &manifest,
        &[
            ("skills/demo/SKILL.md", skill),
            ("skills/demo/OTHER.md", skill),
        ],
    );
    let mut bytes = bytes;
    replace_all_same_length(&mut bytes, b"OTHER.md", b"SKILL.md");
    assert_code(bytes, "DUPLICATE_ARCHIVE_ENTRY");
}

#[test]
fn rejects_symlinks_and_trailing_or_prefixed_bytes() {
    let skill = b"skill";
    let manifest = manifest(vec![file("SKILL.md", skill, false)]);
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    writer.start_file(MANIFEST_PATH, options).unwrap();
    writer
        .write_all(&serde_json::to_vec(&manifest).unwrap())
        .unwrap();
    writer
        .add_symlink("skills/demo/SKILL.md", "outside", options)
        .unwrap();
    assert_code(
        writer.finish().unwrap().into_inner(),
        "UNSAFE_ARCHIVE_ENTRY",
    );

    let valid = bundle(&manifest, &[("SKILL.md", skill, false)]);
    let mut trailing = valid.clone();
    trailing.push(0);
    assert_code(trailing, "INVALID_BUNDLE");
    let mut prefixed = vec![0];
    prefixed.extend(valid);
    assert_code(prefixed, "INVALID_BUNDLE");
}

#[test]
fn rejects_missing_manifest_and_invalid_directory_names() {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .start_file("skills/demo/SKILL.md", SimpleFileOptions::default())
        .unwrap();
    writer.write_all(b"skill").unwrap();
    assert_code(
        writer.finish().unwrap().into_inner(),
        "BUNDLE_MANIFEST_MISSING",
    );

    let skill = b"skill";
    for name in [
        "",
        ".",
        "..",
        "nested/demo",
        "demo\\other",
        "CON",
        "aux.txt",
        "trailing.",
        "bad:name",
    ] {
        let mut manifest = manifest(vec![file("SKILL.md", skill, false)]);
        manifest.skills[0].directory_name = name.into();
        assert_code(
            bundle(&manifest, &[("SKILL.md", skill, false)]),
            "INVALID_BUNDLE_MANIFEST",
        );
    }
}

#[test]
fn rejects_windows_drive_reserved_and_invalid_file_components() {
    let skill = b"skill";
    for path in [
        "C:/secret.txt",
        "CON.txt",
        "nested/COM1.log",
        "bad?.txt",
        "trailing ",
    ] {
        let files = vec![file("SKILL.md", skill, false), file(path, b"data", false)];
        let manifest = manifest_with_untrusted_files(files);
        assert_code(
            bundle(
                &manifest,
                &[("SKILL.md", skill, false), (path, b"data", false)],
            ),
            "INVALID_BUNDLE_MANIFEST",
        );
    }
}

#[test]
fn rejects_local_central_mismatches_encryption_and_unsupported_methods() {
    let skill = b"skill";
    let manifest = manifest(vec![file("SKILL.md", skill, false)]);
    let valid = raw_bundle(&manifest, &[("skills/demo/SKILL.md", skill)]);
    let locals = signature_offsets(&valid, LOCAL_FILE_HEADER);
    let centrals = signature_offsets(&valid, CENTRAL_DIRECTORY_HEADER);
    assert_eq!(locals.len(), 2);
    assert_eq!(centrals.len(), 2);

    let mut mismatch = valid.clone();
    write_u16(&mut mismatch, locals[1] + 8, 8);
    assert_code(mismatch, "INVALID_BUNDLE");

    let mut encrypted = valid.clone();
    write_u16(&mut encrypted, locals[1] + 6, 1);
    write_u16(&mut encrypted, centrals[1] + 8, 1);
    assert_code(encrypted, "UNSUPPORTED_ARCHIVE_FEATURE");

    let mut unsupported = valid.clone();
    write_u16(&mut unsupported, locals[1] + 8, 99);
    write_u16(&mut unsupported, centrals[1] + 10, 99);
    assert_code(unsupported, "UNSUPPORTED_ARCHIVE_FEATURE");

    let mut descriptor = valid.clone();
    write_u16(&mut descriptor, locals[1] + 6, 1 << 3);
    write_u16(&mut descriptor, centrals[1] + 8, 1 << 3);
    assert_code(descriptor, "UNSUPPORTED_ARCHIVE_FEATURE");

    let mut changed_name = valid.clone();
    changed_name[locals[1] + LOCAL_HEADER_SIZE as usize] ^= 1;
    assert_code(changed_name, "INVALID_BUNDLE");

    let mut changed_flags = valid.clone();
    write_u16(&mut changed_flags, locals[1] + 6, UTF8_FLAG);
    assert_code(changed_flags, "INVALID_BUNDLE");

    for field in [14, 18, 22] {
        let mut bytes = valid.clone();
        let value = read_u32(&bytes, locals[1] + field);
        write_u32(&mut bytes, locals[1] + field, value + 1);
        assert_code(bytes, "INVALID_BUNDLE");
    }

    let mut central_name_overflow = valid.clone();
    write_u16(&mut central_name_overflow, centrals[1] + 28, u16::MAX);
    assert_code(central_name_overflow, "INVALID_BUNDLE");

    let mut local_name_overflow = valid;
    write_u16(&mut local_name_overflow, locals[1] + 26, u16::MAX);
    assert_code(local_name_overflow, "INVALID_BUNDLE");
}

#[test]
fn rejects_crc_corruption_non_utf8_names_and_special_file_modes() {
    let skill = b"skill";
    let manifest = manifest(vec![file("SKILL.md", skill, false)]);
    let valid = raw_bundle(&manifest, &[("skills/demo/SKILL.md", skill)]);
    let locals = signature_offsets(&valid, LOCAL_FILE_HEADER);
    let centrals = signature_offsets(&valid, CENTRAL_DIRECTORY_HEADER);

    let mut corrupt = valid.clone();
    let name_length = read_u16(&corrupt, locals[1] + 26) as usize;
    let extra_length = read_u16(&corrupt, locals[1] + 28) as usize;
    let data_start = locals[1] + LOCAL_HEADER_SIZE as usize + name_length + extra_length;
    corrupt[data_start] ^= 1;
    assert_code(corrupt, "INVALID_BUNDLE");

    let mut non_utf8 = valid.clone();
    let local_name = locals[1] + LOCAL_HEADER_SIZE as usize;
    let central_name = centrals[1] + CENTRAL_HEADER_SIZE as usize;
    non_utf8[local_name] = 0xff;
    non_utf8[central_name] = 0xff;
    assert_code(non_utf8, "UNSAFE_ARCHIVE_ENTRY");

    let mut special = valid;
    write_u32(&mut special, centrals[1] + 38, 0o010000 << 16);
    assert_code(special, "UNSAFE_ARCHIVE_ENTRY");

    let mut dos_directory = raw_bundle(&manifest, &[("skills/demo/SKILL.md", skill)]);
    let central = signature_offsets(&dos_directory, CENTRAL_DIRECTORY_HEADER)[1];
    let attributes = read_u32(&dos_directory, central + 38);
    write_u32(&mut dos_directory, central + 38, attributes | 0x10);
    assert_code(dos_directory, "UNSAFE_ARCHIVE_ENTRY");
}

#[test]
fn rejects_zip64_extra_fields_and_declared_resource_excess() {
    let skill = b"skill";
    let manifest = manifest(vec![file("SKILL.md", skill, false)]);
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .large_file(true);
    writer.start_file(MANIFEST_PATH, options).unwrap();
    writer
        .write_all(&serde_json::to_vec(&manifest).unwrap())
        .unwrap();
    writer
        .start_file("skills/demo/SKILL.md", options)
        .unwrap();
    writer.write_all(skill).unwrap();
    assert_code(
        writer.finish().unwrap().into_inner(),
        "UNSUPPORTED_ARCHIVE_FEATURE",
    );

    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    writer.set_raw_zip64_comment(Some(Vec::new().into_boxed_slice()));
    writer
        .start_file(MANIFEST_PATH, SimpleFileOptions::default())
        .unwrap();
    writer
        .write_all(&serde_json::to_vec(&manifest).unwrap())
        .unwrap();
    writer
        .start_file("skills/demo/SKILL.md", SimpleFileOptions::default())
        .unwrap();
    writer.write_all(skill).unwrap();
    let zip64 = writer.finish().unwrap().into_inner();
    assert!(!signature_offsets(&zip64, 0x0606_4b50).is_empty());
    assert!(!signature_offsets(&zip64, 0x0706_4b50).is_empty());
    assert_code(zip64, "INVALID_BUNDLE");

    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let mut options = FullFileOptions::default().compression_method(CompressionMethod::Stored);
    options
        .add_extra_data(0xcafe, vec![1, 2].into_boxed_slice(), false)
        .unwrap();
    writer.start_file(MANIFEST_PATH, options.clone()).unwrap();
    writer
        .write_all(&serde_json::to_vec(&manifest).unwrap())
        .unwrap();
    writer
        .start_file("skills/demo/SKILL.md", options)
        .unwrap();
    writer.write_all(skill).unwrap();
    assert_code(
        writer.finish().unwrap().into_inner(),
        "UNSUPPORTED_ARCHIVE_FEATURE",
    );

    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let mut unicode_path = vec![1];
    unicode_path.extend_from_slice(&crc32fast::hash(MANIFEST_PATH.as_bytes()).to_le_bytes());
    unicode_path.extend_from_slice(b"alternate-manifest.json");
    let mut unicode_options =
        FullFileOptions::default().compression_method(CompressionMethod::Stored);
    unicode_options
        .add_extra_data(0xcafe, unicode_path.into_boxed_slice(), false)
        .unwrap();
    writer.start_file(MANIFEST_PATH, unicode_options).unwrap();
    writer
        .write_all(&serde_json::to_vec(&manifest).unwrap())
        .unwrap();
    writer
        .start_file("skills/demo/SKILL.md", SimpleFileOptions::default())
        .unwrap();
    writer.write_all(skill).unwrap();
    let mut unicode_extra = writer.finish().unwrap().into_inner();
    replace_all_same_length(&mut unicode_extra, &[0xfe, 0xca], &[0x75, 0x70]);
    let mut reference_reader = ZipArchive::new(Cursor::new(unicode_extra.clone())).unwrap();
    assert_eq!(
        reference_reader.by_index(0).unwrap().name(),
        "alternate-manifest.json"
    );
    assert_code(unicode_extra, "UNSUPPORTED_ARCHIVE_FEATURE");

    let mut oversized = manifest;
    oversized.skills[0].files[0].size = MAX_FILE_BYTES + 1;
    oversized.skills[0].revision = skill_revision(&oversized.skills[0].files).unwrap();
    assert_code(
        bundle(&oversized, &[("SKILL.md", skill, false)]),
        "BUNDLE_LIMIT_EXCEEDED",
    );
}

#[test]
fn rejects_malformed_multidisk_comments_directories_overlaps_and_consistent_prefixes() {
    let skill = b"skill";
    let manifest = manifest(vec![file("SKILL.md", skill, false)]);
    let valid = raw_bundle(&manifest, &[("skills/demo/SKILL.md", skill)]);

    for length in [0, 10, valid.len() - 1, valid.len() / 2] {
        assert_code(valid[..length].to_vec(), "INVALID_BUNDLE");
    }

    let mut multidisk = valid.clone();
    let eocd = signature_offsets(&multidisk, END_OF_CENTRAL_DIRECTORY)[0];
    write_u16(&mut multidisk, eocd + 4, 1);
    assert_code(multidisk, "UNSUPPORTED_ARCHIVE_FEATURE");

    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    writer.set_comment("comment");
    writer
        .start_file(MANIFEST_PATH, SimpleFileOptions::default())
        .unwrap();
    writer
        .write_all(&serde_json::to_vec(&manifest).unwrap())
        .unwrap();
    writer
        .start_file("skills/demo/SKILL.md", SimpleFileOptions::default())
        .unwrap();
    writer.write_all(skill).unwrap();
    assert_code(
        writer.finish().unwrap().into_inner(),
        "INVALID_BUNDLE",
    );

    let mut entry_comment = valid.clone();
    let central = signature_offsets(&entry_comment, CENTRAL_DIRECTORY_HEADER)[0];
    write_u16(&mut entry_comment, central + 32, 1);
    assert_code(entry_comment, "UNSUPPORTED_ARCHIVE_FEATURE");

    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .start_file(MANIFEST_PATH, SimpleFileOptions::default())
        .unwrap();
    writer
        .write_all(&serde_json::to_vec(&manifest).unwrap())
        .unwrap();
    writer
        .add_directory("skills/demo/", SimpleFileOptions::default())
        .unwrap();
    writer
        .start_file("skills/demo/SKILL.md", SimpleFileOptions::default())
        .unwrap();
    writer.write_all(skill).unwrap();
    assert_code(writer.finish().unwrap().into_inner(), "UNSAFE_ARCHIVE_ENTRY");

    let mut overlap = valid.clone();
    let locals = signature_offsets(&overlap, LOCAL_FILE_HEADER);
    let centrals = signature_offsets(&overlap, CENTRAL_DIRECTORY_HEADER);
    let compressed = read_u32(&overlap, locals[0] + 18);
    write_u32(&mut overlap, locals[0] + 18, compressed + 1);
    write_u32(&mut overlap, centrals[0] + 20, compressed + 1);
    assert_code(overlap, "INVALID_BUNDLE");

    let mut prefixed = Vec::with_capacity(valid.len() + 1);
    prefixed.push(0);
    prefixed.extend_from_slice(&valid);
    let centrals = signature_offsets(&prefixed, CENTRAL_DIRECTORY_HEADER);
    for central in centrals {
        let offset = read_u32(&prefixed, central + 42);
        write_u32(&mut prefixed, central + 42, offset + 1);
    }
    let eocd = signature_offsets(&prefixed, END_OF_CENTRAL_DIRECTORY)[0];
    let central_offset = read_u32(&prefixed, eocd + 16);
    write_u32(&mut prefixed, eocd + 16, central_offset + 1);
    assert_code(prefixed, "INVALID_BUNDLE");
}

#[test]
fn rejects_schema_identity_digest_and_directory_collisions() {
    let skill = b"skill";
    let valid_file = file("SKILL.md", skill, false);

    let mut wrong_format = manifest(vec![valid_file.clone()]);
    wrong_format.format = "other/format".into();
    assert_code(
        bundle(&wrong_format, &[("SKILL.md", skill, false)]),
        "INVALID_BUNDLE_MANIFEST",
    );

    let mut wrong_contract = manifest(vec![valid_file.clone()]);
    wrong_contract.agent_contract.id = "other".into();
    assert_code(
        bundle(&wrong_contract, &[("SKILL.md", skill, false)]),
        "UNSUPPORTED_BUNDLE_VERSION",
    );

    for digest in ["A".repeat(64), "f".repeat(63), "g".repeat(64)] {
        let mut value = manifest(vec![valid_file.clone()]);
        value.skills[0].files[0].sha256 = digest;
        value.skills[0].revision = "f".repeat(64);
        assert_code(
            bundle(&value, &[("SKILL.md", skill, false)]),
            "INVALID_BUNDLE_MANIFEST",
        );
    }

    let alpha = manifest(vec![valid_file.clone()]).skills.remove(0);
    let mut duplicate = BundleManifest {
        format: BUNDLE_FORMAT.into(),
        format_version: BUNDLE_FORMAT_VERSION,
        agent_contract: AgentContract {
            id: CODEX_CONTRACT_ID.into(),
            version: CODEX_CONTRACT_VERSION,
        },
        skills: vec![alpha.clone(), alpha],
    };
    assert!(matches!(
        validate_manifest(&duplicate),
        Err(BundleError::InvalidManifest)
    ));
    duplicate.skills[1].directory_name = "DEMO".into();
    duplicate.skills.swap(0, 1);
    assert!(matches!(
        validate_manifest(&duplicate),
        Err(BundleError::InvalidManifest)
    ));

    let mut normalized = duplicate.clone();
    normalized.skills[0].directory_name = "e\u{301}".into();
    normalized.skills[1].directory_name = "é".into();
    assert!(matches!(
        validate_manifest(&normalized),
        Err(BundleError::InvalidManifest)
    ));

    let missing_root = manifest_with_untrusted_files(vec![file("other.md", b"x", false)]);
    assert!(matches!(
        validate_manifest(&missing_root),
        Err(BundleError::InvalidManifest)
    ));

    let duplicate_files = manifest_with_untrusted_files(vec![
        file("SKILL.md", skill, false),
        file("SKILL.md", skill, false),
    ]);
    assert!(matches!(
        validate_manifest(&duplicate_files),
        Err(BundleError::InvalidManifest)
    ));
}

#[test]
fn rejects_complete_entries_outside_canonical_archive_order() {
    let skill = b"skill";
    let files = vec![
        file("SKILL.md", skill, false),
        file("z.txt", b"z", false),
    ];
    let manifest = manifest(files);
    let reordered = raw_bundle(
        &manifest,
        &[
            ("skills/demo/z.txt", b"z"),
            ("skills/demo/SKILL.md", skill),
        ],
    );
    assert_code(reordered, "INVALID_BUNDLE");

    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    writer
        .start_file("skills/demo/SKILL.md", options)
        .unwrap();
    writer.write_all(skill).unwrap();
    writer.start_file(MANIFEST_PATH, options).unwrap();
    writer
        .write_all(&serde_json::to_vec(&manifest).unwrap())
        .unwrap();
    assert_code(writer.finish().unwrap().into_inner(), "INVALID_BUNDLE");
}

#[test]
fn enforces_all_manifest_and_archive_resource_limits() {
    assert_code(
        vec![0; (MAX_ARCHIVE_BYTES as usize).min(1)],
        "INVALID_BUNDLE",
    );
    let mut large_reader = LengthOnlyReader {
        length: MAX_ARCHIVE_BYTES + 1,
        position: 0,
    };
    assert_eq!(
        inspect_bundle(&mut large_reader).unwrap_err().code(),
        "BUNDLE_LIMIT_EXCEEDED"
    );

    let skill = b"skill";
    let base = manifest(vec![file("SKILL.md", skill, false)]);
    let mut declared_manifest = raw_bundle(&base, &[("skills/demo/SKILL.md", skill)]);
    let locals = signature_offsets(&declared_manifest, LOCAL_FILE_HEADER);
    let centrals = signature_offsets(&declared_manifest, CENTRAL_DIRECTORY_HEADER);
    write_u32(
        &mut declared_manifest,
        locals[0] + 22,
        MAX_MANIFEST_BYTES as u32 + 1,
    );
    write_u32(
        &mut declared_manifest,
        centrals[0] + 24,
        MAX_MANIFEST_BYTES as u32 + 1,
    );
    assert_code(declared_manifest, "BUNDLE_LIMIT_EXCEEDED");

    let too_many_skills = manifest_with_skill_shapes(MAX_SKILLS + 1, 1, 0);
    assert_limit(too_many_skills);
    let too_many_files = manifest_with_skill_shapes(1, MAX_FILES_PER_SKILL + 1, 0);
    assert_limit(too_many_files);
    let too_many_total_files = manifest_with_skill_shapes(17, 512, 0);
    assert_limit(too_many_total_files);
    let too_large_skill = manifest_with_skill_shapes(1, 5, 16 * 1024 * 1024);
    assert_limit(too_large_skill);
    let too_large_total = manifest_with_skill_shapes(9, 4, 16 * 1024 * 1024);
    assert_limit(too_large_total);

    assert!(validate_relative_path(&"a".repeat(MAX_COMPONENT_BYTES)).is_ok());
    assert!(validate_relative_path(&"a".repeat(MAX_COMPONENT_BYTES + 1)).is_err());
    assert!(validate_relative_path(&vec!["a"; MAX_PATH_DEPTH].join("/")).is_ok());
    assert!(validate_relative_path(&vec!["a"; MAX_PATH_DEPTH + 1].join("/")).is_err());
    let exact_path = [
        "a".repeat(255),
        "a".repeat(255),
        "a".repeat(255),
        "a".repeat(254),
        "a".into(),
    ]
    .join("/");
    let oversized_path = [
        "a".repeat(255),
        "a".repeat(255),
        "a".repeat(255),
        "a".repeat(254),
        "aa".into(),
    ]
    .join("/");
    assert_eq!(exact_path.len(), MAX_PATH_BYTES);
    assert_eq!(oversized_path.len(), MAX_PATH_BYTES + 1);
    assert!(validate_relative_path(&exact_path).is_ok());
    assert!(validate_relative_path(&oversized_path).is_err());
}

#[test]
fn streamed_output_cannot_exceed_manifest_size_even_when_zip_headers_lie() {
    let actual = b"0123456789";
    let declared = BundleFile {
        path: "SKILL.md".into(),
        size: 5,
        sha256: format!("{:x}", Sha256::digest(&actual[..5])),
        executable: false,
    };
    let manifest = manifest(vec![declared]);
    let mut bytes = raw_bundle(&manifest, &[("skills/demo/SKILL.md", actual)]);
    let locals = signature_offsets(&bytes, LOCAL_FILE_HEADER);
    let centrals = signature_offsets(&bytes, CENTRAL_DIRECTORY_HEADER);
    write_u32(&mut bytes, locals[1] + 22, 5);
    write_u32(&mut bytes, centrals[1] + 24, 5);
    assert_code(bytes, "BUNDLE_LIMIT_EXCEEDED");
}

fn raw_bundle(manifest: &BundleManifest, entries: &[(&str, &[u8])]) -> Vec<u8> {
    full_bundle(manifest, entries, CompressionMethod::Stored)
}

fn full_bundle(
    manifest: &BundleManifest,
    entries: &[(&str, &[u8])],
    method: CompressionMethod,
) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(method);
    writer.start_file(MANIFEST_PATH, options).unwrap();
    writer
        .write_all(&serde_json::to_vec(manifest).unwrap())
        .unwrap();
    for (path, contents) in entries {
        writer.start_file(*path, options).unwrap();
        writer.write_all(contents).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn signature_offsets(bytes: &[u8], signature: u32) -> Vec<usize> {
    let signature = signature.to_le_bytes();
    bytes
        .windows(signature.len())
        .enumerate()
        .filter_map(|(index, window)| (window == signature).then_some(index))
        .collect()
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn write_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn manifest_with_skill_shapes(
    skill_count: usize,
    files_per_skill: usize,
    file_size: u64,
) -> BundleManifest {
    let mut skills = Vec::with_capacity(skill_count);
    for skill_index in 0..skill_count {
        let mut files = Vec::with_capacity(files_per_skill);
        if files_per_skill > 0 {
            files.push(BundleFile {
                path: "SKILL.md".into(),
                size: file_size,
                sha256: format!("{:x}", Sha256::digest([])),
                executable: false,
            });
        }
        for file_index in 1..files_per_skill {
            files.push(BundleFile {
                path: format!("file-{file_index:05}.bin"),
                size: file_size,
                sha256: format!("{:x}", Sha256::digest([])),
                executable: false,
            });
        }
        let revision = skill_revision(&files).unwrap_or_else(|_| "f".repeat(64));
        skills.push(BundleSkill {
            directory_name: format!("skill-{skill_index:05}"),
            revision,
            files,
        });
    }
    BundleManifest {
        format: BUNDLE_FORMAT.into(),
        format_version: BUNDLE_FORMAT_VERSION,
        agent_contract: AgentContract {
            id: CODEX_CONTRACT_ID.into(),
            version: CODEX_CONTRACT_VERSION,
        },
        skills,
    }
}

fn assert_limit(manifest: BundleManifest) {
    assert!(matches!(
        validate_manifest(&manifest),
        Err(BundleError::LimitExceeded)
    ));
}

struct LengthOnlyReader {
    length: u64,
    position: u64,
}

impl Read for LengthOnlyReader {
    fn read(&mut self, _buffer: &mut [u8]) -> std::io::Result<usize> {
        Ok(0)
    }
}

impl Seek for LengthOnlyReader {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        let next = match position {
            SeekFrom::Start(value) => i128::from(value),
            SeekFrom::End(value) => i128::from(self.length) + i128::from(value),
            SeekFrom::Current(value) => i128::from(self.position) + i128::from(value),
        };
        if !(0..=i128::from(u64::MAX)).contains(&next) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "invalid seek",
            ));
        }
        self.position = next as u64;
        Ok(self.position)
    }
}

fn replace_manifest(bytes: &mut Vec<u8>, manifest_bytes: Vec<u8>) {
    let skill = b"skill";
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    writer.start_file(MANIFEST_PATH, options).unwrap();
    writer.write_all(&manifest_bytes).unwrap();
    writer
        .start_file("skills/demo/SKILL.md", options)
        .unwrap();
    writer.write_all(skill).unwrap();
    *bytes = writer.finish().unwrap().into_inner();
}

fn replace_all_same_length(bytes: &mut [u8], from: &[u8], to: &[u8]) {
    assert_eq!(from.len(), to.len());
    for index in 0..=bytes.len() - from.len() {
        if &bytes[index..index + from.len()] == from {
            bytes[index..index + from.len()].copy_from_slice(to);
        }
    }
}

fn assert_code(bytes: Vec<u8>, expected: &str) {
    let error = inspect_bundle(&mut Cursor::new(bytes)).unwrap_err();
    assert_eq!(error.code(), expected, "unexpected error: {error}");
}
