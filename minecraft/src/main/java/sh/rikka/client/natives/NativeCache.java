package sh.rikka.client.natives;

import java.io.IOException;
import java.io.InputStream;
import java.io.UncheckedIOException;
import java.nio.file.DirectoryStream;
import java.nio.file.FileSystemException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.time.Duration;
import java.time.Instant;
import java.util.Comparator;
import java.util.regex.Pattern;
import java.util.stream.Stream;
import java.util.zip.ZipEntry;
import java.util.zip.ZipInputStream;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

final class NativeCache {
	private static final Pattern DIGEST = Pattern.compile("[0-9a-f]{64}");
	private static final Logger LOGGER = LoggerFactory.getLogger(NativeCache.class);
	private static final String COMPLETE = ".complete";
	private static final Pattern STAGING = Pattern.compile("[0-9a-f]{64}-\\d+");
	private static final Duration ABANDONED = Duration.ofDays(1);

	private final Path root;
	private final String library;

	NativeCache(Path root, String library) {
		this.root = root;
		this.library = library;
	}

	Path unpack(String digest, Archive archive) {
		if (!DIGEST.matcher(digest).matches()) {
			throw new IllegalArgumentException(digest + " is not a SHA-256 digest");
		}
		try {
			Files.createDirectories(root);
			Path unpacked = root.resolve(digest);
			if (!isComplete(unpacked)) {
				place(archive, unpacked, digest);
			}
			prune(digest);
			return unpacked.resolve(library);
		} catch (IOException failure) {
			throw new UncheckedIOException("failed to unpack native libraries into " + root, failure);
		}
	}

	private boolean isComplete(Path unpacked) {
		return Files.isRegularFile(unpacked.resolve(COMPLETE)) && Files.isRegularFile(unpacked.resolve(library));
	}

	private void place(Archive archive, Path unpacked, String digest) throws IOException {
		Path staging = Files.createTempDirectory(root, digest + "-");
		try {
			extract(archive, staging);
			Files.createFile(staging.resolve(COMPLETE));
			if (isComplete(unpacked)) {
				return;
			}
			if (Files.exists(unpacked)) {
				delete(unpacked);
			}
			Files.move(staging, unpacked, StandardCopyOption.ATOMIC_MOVE);
		} catch (FileSystemException failure) {
			if (!isComplete(unpacked)) {
				throw failure;
			}
		} finally {
			if (Files.exists(staging)) {
				delete(staging);
			}
		}
	}

	private static void extract(Archive archive, Path staging) throws IOException {
		try (ZipInputStream zip = new ZipInputStream(archive.open())) {
			for (ZipEntry entry = zip.getNextEntry(); entry != null; entry = zip.getNextEntry()) {
				Path target = staging.resolve(entry.getName()).normalize();
				if (!target.startsWith(staging) || target.equals(staging)) {
					throw new IOException("archive entry " + entry.getName() + " leaves its folder");
				}
				if (entry.isDirectory()) {
					Files.createDirectories(target);
				} else {
					Files.createDirectories(target.getParent());
					Files.copy(zip, target);
				}
			}
		}
	}

	private void prune(String current) throws IOException {
		Instant abandoned = Instant.now().minus(ABANDONED);
		try (DirectoryStream<Path> entries = Files.newDirectoryStream(root)) {
			for (Path entry : entries) {
				String name = entry.getFileName().toString();
				if (!name.equals(current) && DIGEST.matcher(name).matches()) {
					discard(entry);
				} else if (STAGING.matcher(name).matches()
						&& Files.getLastModifiedTime(entry).toInstant().isBefore(abandoned)) {
					deleteOutdated(entry);
				}
			}
		}
	}

	private void discard(Path unpacked) {
		try {
			Files.deleteIfExists(unpacked.resolve(library));
		} catch (IOException loaded) {
			LOGGER.info("Keeping {}, which another running game loads", unpacked);
			return;
		}
		deleteOutdated(unpacked);
	}

	private static void deleteOutdated(Path tree) {
		try {
			delete(tree);
		} catch (IOException failure) {
			LOGGER.warn("Failed to delete outdated native libraries {}", tree, failure);
		}
	}

	private static void delete(Path tree) throws IOException {
		try (Stream<Path> paths = Files.walk(tree)) {
			for (Path path : paths.sorted(Comparator.reverseOrder()).toList()) {
				Files.delete(path);
			}
		}
	}

	@FunctionalInterface
	interface Archive {
		InputStream open() throws IOException;
	}
}
