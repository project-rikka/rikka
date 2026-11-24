package sh.rikka.client.resource.legacy;

import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Optional;
import java.util.Set;
import java.util.zip.ZipEntry;
import java.util.zip.ZipFile;

public interface PackArchive extends AutoCloseable {
	static PackArchive open(Path path) throws IOException {
		return Files.isDirectory(path) ? DirectoryArchive.open(path) : ZipArchive.open(path);
	}

	static Optional<byte[]> readRoot(Path path, String name) throws IOException {
		if (Files.isDirectory(path)) {
			Path file = path.resolve(name);
			return Files.isRegularFile(file) ? Optional.of(Files.readAllBytes(file)) : Optional.empty();
		}
		try (ZipFile zip = new ZipFile(path.toFile())) {
			ZipEntry entry = zip.getEntry(name);
			if (entry == null) {
				return Optional.empty();
			}
			try (InputStream in = zip.getInputStream(entry)) {
				return Optional.of(in.readAllBytes());
			}
		}
	}

	Set<String> entries();

	byte[] read(String entry) throws IOException;

	@Override
	void close() throws IOException;

	static String normalize(String entry) {
		String path = entry.replace('\\', '/');
		while (path.startsWith("/") || path.startsWith("./")) {
			path = path.substring(path.charAt(0) == '/' ? 1 : 2);
		}
		return path;
	}
}
