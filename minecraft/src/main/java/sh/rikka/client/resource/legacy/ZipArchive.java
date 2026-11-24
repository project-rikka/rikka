package sh.rikka.client.resource.legacy;

import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Path;
import java.util.Collections;
import java.util.Enumeration;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Set;
import java.util.zip.ZipEntry;
import java.util.zip.ZipFile;

final class ZipArchive implements PackArchive {
	private final ZipFile zip;
	private final Map<String, ZipEntry> entries;

	private ZipArchive(ZipFile zip, Map<String, ZipEntry> entries) {
		this.zip = zip;
		this.entries = entries;
	}

	static ZipArchive open(Path path) throws IOException {
		ZipFile zip = new ZipFile(path.toFile());
		Map<String, ZipEntry> entries = new LinkedHashMap<>();
		Enumeration<? extends ZipEntry> all = zip.entries();
		while (all.hasMoreElements()) {
			ZipEntry entry = all.nextElement();
			if (!entry.isDirectory()) {
				entries.putIfAbsent(PackArchive.normalize(entry.getName()), entry);
			}
		}
		return new ZipArchive(zip, entries);
	}

	@Override
	public Set<String> entries() {
		return Collections.unmodifiableSet(this.entries.keySet());
	}

	@Override
	public byte[] read(String entry) throws IOException {
		ZipEntry zipEntry = this.entries.get(entry);
		if (zipEntry == null) {
			throw new IOException("missing pack entry " + entry);
		}
		try (InputStream in = this.zip.getInputStream(zipEntry)) {
			return in.readAllBytes();
		}
	}

	@Override
	public void close() throws IOException {
		this.zip.close();
	}
}
