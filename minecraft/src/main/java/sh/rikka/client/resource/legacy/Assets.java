package sh.rikka.client.resource.legacy;

import com.google.gson.JsonElement;
import java.io.IOException;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Map;
import java.util.NavigableMap;
import java.util.Optional;
import java.util.TreeMap;

public final class Assets {
	public static final String ROOT = "assets/";
	static final String MINECRAFT = ROOT + "minecraft/";
	static final String TEXTURES = MINECRAFT + "textures/";
	static final String PNG = ".png";
	static final String MCMETA = ".mcmeta";

	private final TreeMap<String, Asset> entries = new TreeMap<>();
	private final ImageDecoder decoder;

	public Assets(ImageDecoder decoder) {
		this.decoder = decoder;
	}

	static String texture(String id) {
		return TEXTURES + id + PNG;
	}

	static String textureMeta(String id) {
		return texture(id) + MCMETA;
	}

	static String id(String path, String prefix, String extension) {
		return path.substring(prefix.length(), path.length() - extension.length());
	}

	ImageDecoder decoder() {
		return this.decoder;
	}

	boolean has(String path) {
		return this.entries.containsKey(path);
	}

	public Optional<Asset> get(String path) {
		return Optional.ofNullable(this.entries.get(path));
	}

	void put(String path, Asset asset) {
		this.entries.put(path, asset);
	}

	Optional<Asset> remove(String path) {
		return Optional.ofNullable(this.entries.remove(path));
	}

	public List<String> paths(String prefix) {
		NavigableMap<String, Asset> tail = this.entries.tailMap(prefix, true);
		List<String> result = new ArrayList<>();
		for (String path : tail.keySet()) {
			if (!path.startsWith(prefix)) {
				break;
			}
			result.add(path);
		}
		return result;
	}

	public Map<String, Asset> view() {
		return Collections.unmodifiableMap(this.entries);
	}

	boolean hasTexture(String id) {
		return has(texture(id));
	}

	Optional<Asset> findTexture(String id) {
		return Optional.ofNullable(this.entries.get(texture(id)));
	}

	Raster textureRaster(String id) throws IOException {
		Asset asset = this.entries.get(texture(id));
		if (asset == null) {
			throw new IOException("missing texture " + id);
		}
		return asset.raster(this.decoder);
	}

	void putTexture(String id, RasterSource image) {
		this.entries.put(texture(id), Asset.image(image));
	}

	void putJson(String path, JsonElement json) {
		this.entries.put(path, Asset.encoded(() -> Text.encode(json)));
	}

	void moveTexture(String from, String to) {
		move(texture(from), texture(to));
		move(textureMeta(from), textureMeta(to));
	}

	void removeTexture(String id) {
		this.entries.remove(texture(id));
		this.entries.remove(textureMeta(id));
	}

	void move(String from, String to) {
		Asset asset = this.entries.remove(from);
		if (asset != null) {
			this.entries.put(to, asset);
		}
	}
}
