package sh.rikka.client.natives;

import java.io.FileNotFoundException;
import java.io.IOException;
import java.io.InputStream;
import java.io.UncheckedIOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.util.Locale;
import java.util.Set;
import net.fabricmc.loader.api.entrypoint.PreLaunchEntrypoint;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import os.rikka.Rikka;

public final class NativeLibraries implements PreLaunchEntrypoint {
	private static final Logger LOGGER = LoggerFactory.getLogger(NativeLibraries.class);
	private static final String WINDOWS_ARCHIVE = "/natives/windows-x86_64.zip";
	private static final String DIGEST_SUFFIX = ".sha256";
	private static final String WINDOWS_LIBRARY = "rikka.dll";
	private static final Set<String> X86_64 = Set.of("amd64", "x86_64");
	private static final String CACHE_ROOT = "LOCALAPPDATA";

	@Override
	public void onPreLaunch() {
		if (!System.getProperty("os.name").startsWith("Windows")) {
			return;
		}
		String architecture = System.getProperty("os.arch").toLowerCase(Locale.ROOT);
		if (!X86_64.contains(architecture)) {
			throw new IllegalStateException("Rikka Client runs on x86-64 Windows only, not " + architecture);
		}
		String localData = System.getenv(CACHE_ROOT);
		if (localData == null || localData.isEmpty()) {
			throw new IllegalStateException("%" + CACHE_ROOT + "% is not set, so Rikka Client has nowhere to unpack its libraries");
		}
		NativeCache cache = new NativeCache(Path.of(localData, "rikka", "cache", "natives"), WINDOWS_LIBRARY);
		Path library = cache.unpack(digest(), () -> resource(WINDOWS_ARCHIVE));
		System.setProperty(Rikka.LIBRARY_PROPERTY, library.toString());
		LOGGER.info("Loading Rikka from {}", library);
	}

	private static String digest() {
		try (InputStream digest = resource(WINDOWS_ARCHIVE + DIGEST_SUFFIX)) {
			return new String(digest.readAllBytes(), StandardCharsets.US_ASCII).strip();
		} catch (IOException failure) {
			throw new UncheckedIOException("failed to read the digest of " + WINDOWS_ARCHIVE, failure);
		}
	}

	private static InputStream resource(String name) throws FileNotFoundException {
		InputStream stream = NativeLibraries.class.getResourceAsStream(name);
		if (stream == null) {
			throw new FileNotFoundException("the mod jar holds no " + name);
		}
		return stream;
	}
}
