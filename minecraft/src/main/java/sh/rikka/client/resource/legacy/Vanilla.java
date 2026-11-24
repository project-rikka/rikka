package sh.rikka.client.resource.legacy;

import com.google.gson.JsonElement;
import java.io.IOException;
import java.util.HashMap;
import java.util.Map;
import java.util.Optional;

public final class Vanilla {
	private final Reader reader;
	private final ImageDecoder decoder;
	private final Map<String, Optional<Raster>> rasters = new HashMap<>();

	public Vanilla(Reader reader, ImageDecoder decoder) {
		this.reader = reader;
		this.decoder = decoder;
	}

	Optional<byte[]> read(String path) throws IOException {
		return this.reader.read(path);
	}

	Optional<JsonElement> json(String path) throws IOException {
		Optional<byte[]> bytes = this.reader.read(path);
		return bytes.isPresent() ? Optional.of(Text.json(bytes.get())) : Optional.empty();
	}

	synchronized Optional<Raster> texture(String id) throws IOException {
		String path = Assets.texture(id);
		Optional<Raster> cached = this.rasters.get(path);
		if (cached != null) {
			return cached;
		}
		Optional<byte[]> bytes = this.reader.read(path);
		Optional<Raster> raster = bytes.isPresent() ? Optional.of(this.decoder.decode(bytes.get())) : Optional.empty();
		this.rasters.put(path, raster);
		return raster;
	}

	@FunctionalInterface
	public interface Reader {
		Optional<byte[]> read(String path) throws IOException;
	}
}
