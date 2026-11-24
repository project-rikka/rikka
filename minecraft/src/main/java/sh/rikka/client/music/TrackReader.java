package sh.rikka.client.music;

import java.nio.file.Path;
import java.util.Locale;
import java.util.Set;
import org.jspecify.annotations.Nullable;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import os.rikka.Image;
import os.rikka.SoundTags;

final class TrackReader {
	private static final Logger LOGGER = LoggerFactory.getLogger(TrackReader.class);
	private static final int COVER_SIZE = 128;
	private static final Set<String> EXTENSIONS = Set.of(
		"aac", "aif", "aiff", "ape", "flac", "m4a", "mp3", "mpc", "oga", "ogg", "opus", "wav", "wv"
	);

	private TrackReader() {
	}

	static boolean isAudio(Path file) {
		return EXTENSIONS.contains(FileName.of(file).extension().toLowerCase(Locale.ROOT));
	}

	static Track read(Path folder, Path file) {
		String key = key(folder, file);
		try (SoundTags tags = new SoundTags(file.toString(), COVER_SIZE)) {
			String failure = tags.getErrorMessage();
			if (!failure.isEmpty()) {
				LOGGER.debug("listing {} by its file name: {}", file, failure);
				return new Track(file, key, FileName.of(file).stem(), "", "", "", 0.0, null);
			}
			String title = tags.getTitle().strip();
			@Nullable Image cover = tags.hasCover() ? tags.getCover() : null;
			return new Track(
				file,
				key,
				title.isEmpty() ? FileName.of(file).stem() : title,
				tags.getArtist().strip(),
				tags.getAlbum().strip(),
				tags.getAlbumArtist().strip(),
				tags.getDuration(),
				cover
			);
		}
	}

	private static String key(Path folder, Path file) {
		return folder.relativize(file).toString().replace(file.getFileSystem().getSeparator(), "/");
	}
}
