package sh.rikka.client.music;

import java.nio.file.Path;

record FileName(String stem, String extension) {
	static FileName of(Path file) {
		String name = file.getFileName().toString();
		int dot = name.lastIndexOf('.');
		return dot > 0 ? new FileName(name.substring(0, dot), name.substring(dot + 1)) : new FileName(name, "");
	}

	String withSuffix(String suffix) {
		return this.extension.isEmpty() ? this.stem + suffix : this.stem + suffix + "." + this.extension;
	}
}
