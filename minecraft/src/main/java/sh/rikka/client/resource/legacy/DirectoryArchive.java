package sh.rikka.client.resource.legacy;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Set;
import java.util.stream.Stream;

final class DirectoryArchive implements PackArchive {
	private final Map<String, Path> entries;

	private DirectoryArchive(Map<String, Path> entries) {
		this.entries = entries;
	}

	static DirectoryArchive open(Path root) throws IOException {
		Map<String, Path> entries = new LinkedHashMap<>();
		Path realRoot = root.toRealPath();
		try (Stream<Path> files = Files.walk(realRoot)) {
			files.filter(Files::isRegularFile).sorted().forEach(file -> {
				Path real = toRealPath(file);
				if (real != null && real.startsWith(realRoot)) {
					entries.putIfAbsent(PackArchive.normalize(realRoot.relativize(file).toString()), real);
				}
			});
		}
		return new DirectoryArchive(entries);
	}

	private static Path toRealPath(Path file) {
		try {
			return file.toRealPath();
		} catch (IOException failure) {
			return null;
		}
	}

	@Override
	public Set<String> entries() {
		return Collections.unmodifiableSet(this.entries.keySet());
	}

	@Override
	public byte[] read(String entry) throws IOException {
		Path file = this.entries.get(entry);
		if (file == null) {
			throw new IOException("missing pack entry " + entry);
		}
		return Files.readAllBytes(file);
	}

	@Override
	public void close() {
	}
}
