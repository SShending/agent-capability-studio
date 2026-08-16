export function buildUpdateComparison(localFiles, remoteFiles) {
  const local = new Map(localFiles.map((file) => [file.path, file]));
  const remote = new Map(remoteFiles.map((file) => [file.path, file]));
  return [...new Set([...local.keys(), ...remote.keys()])]
    .sort((left, right) => left.localeCompare(right))
    .map((path) => {
      const localFile = local.get(path) || null;
      const remoteFile = remote.get(path) || null;
      const modeChanged = Boolean(
        localFile && remoteFile && localFile.executable !== remoteFile.executable
      );
      const status = !localFile
        ? "added"
        : !remoteFile
          ? "removed"
          : localFile.sha256 !== remoteFile.sha256 || modeChanged
            ? "modified"
            : "unchanged";
      return { path, local: localFile, remote: remoteFile, modeChanged, status };
    });
}
