package sh.rikka.bench;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import java.io.IOException;
import java.io.UncheckedIOException;
import java.nio.file.Files;
import java.nio.file.Path;
import net.fabricmc.loader.api.FabricLoader;
import net.fabricmc.loader.api.entrypoint.PreLaunchEntrypoint;

public final class BoostProfile implements PreLaunchEntrypoint {
	private static final String OVERRIDES = "RIKKA_BENCH_BOOST";
	private static final String PROFILE = "config/rikka/profiles/default.json";
	private static final String MOD = "boost";
	private static final Gson GSON = new GsonBuilder().setPrettyPrinting().create();

	@Override
	public void onPreLaunch() {
		Path file = FabricLoader.getInstance().getGameDir().resolve(PROFILE);
		String overrides = System.getenv(OVERRIDES);
		try {
			JsonObject profile = Files.exists(file) ? JsonParser.parseString(Files.readString(file)).getAsJsonObject() : new JsonObject();
			JsonObject mods = child(profile, "mods");
			JsonObject boost = child(mods, MOD);
			boost.addProperty("enabled", overrides != null);
			boost.add("settings", overrides == null ? new JsonObject() : settings(overrides));
			Files.createDirectories(file.getParent());
			Files.writeString(file, GSON.toJson(profile));
		} catch (IOException failure) {
			throw new UncheckedIOException("failed to write " + file, failure);
		}
	}

	private static JsonObject settings(String overrides) {
		JsonObject settings = new JsonObject();
		for (String override : overrides.split(",")) {
			if (override.isBlank()) {
				continue;
			}
			int separator = override.indexOf('=');
			if (separator < 0) {
				throw new IllegalArgumentException(OVERRIDES + " entry is not key=value: " + override);
			}
			settings.add(override.substring(0, separator).strip(), JsonParser.parseString(override.substring(separator + 1).strip()));
		}
		return settings;
	}

	private static JsonObject child(JsonObject parent, String name) {
		if (!parent.has(name)) {
			parent.add(name, new JsonObject());
		}
		return parent.getAsJsonObject(name);
	}
}
