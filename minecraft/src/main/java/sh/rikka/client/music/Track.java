package sh.rikka.client.music;

import java.nio.file.Path;
import org.jspecify.annotations.Nullable;
import os.rikka.Image;

public record Track(Path path, String key, String title, String artist, String album, String albumArtist, double duration, @Nullable Image cover) {
	public boolean isSameFile(@Nullable Track other) {
		return other != null && this.path.equals(other.path);
	}

	Track withCover(@Nullable Image shared) {
		return new Track(this.path, this.key, this.title, this.artist, this.album, this.albumArtist, this.duration, shared);
	}

	String albumKey() {
		return this.album.isEmpty() ? "" : (this.albumArtist.isEmpty() ? this.artist : this.albumArtist) + "\u0000" + this.album;
	}
}
