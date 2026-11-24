package sh.rikka.client.resource.legacy;

import com.google.gson.JsonElement;
import java.io.IOException;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.TreeMap;
import java.util.function.Predicate;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

final class Documents {
	private static final Logger LOGGER = LoggerFactory.getLogger(Documents.class);

	private final TreeMap<String, JsonElement> entries = new TreeMap<>();

	public static Documents take(Assets assets, Predicate<String> selected) {
		Documents documents = new Documents();
		for (String path : List.copyOf(assets.view().keySet())) {
			if (!path.endsWith(".json") || !selected.test(path)) {
				continue;
			}
			Asset asset = assets.remove(path).orElseThrow();
			try {
				documents.entries.put(path, Text.json(asset.bytes()));
			} catch (IOException failure) {
				LOGGER.warn("dropped unreadable legacy pack file {}: {}", path, failure.getMessage());
			}
		}
		return documents;
	}

	public void store(Assets assets) {
		for (Map.Entry<String, JsonElement> entry : this.entries.entrySet()) {
			assets.putJson(entry.getKey(), entry.getValue());
		}
	}

	public Optional<JsonElement> get(String path) {
		return Optional.ofNullable(this.entries.get(path));
	}

	public boolean has(String path) {
		return this.entries.containsKey(path);
	}

	public void put(String path, JsonElement json) {
		this.entries.put(path, json);
	}

	public Optional<JsonElement> remove(String path) {
		return Optional.ofNullable(this.entries.remove(path));
	}

	public List<String> paths(String prefix) {
		List<String> result = new ArrayList<>();
		for (String path : this.entries.tailMap(prefix, true).keySet()) {
			if (!path.startsWith(prefix)) {
				break;
			}
			result.add(path);
		}
		return result;
	}

	public List<String> paths(Predicate<String> selected) {
		List<String> result = new ArrayList<>();
		for (String path : this.entries.keySet()) {
			if (selected.test(path)) {
				result.add(path);
			}
		}
		return result;
	}
}
