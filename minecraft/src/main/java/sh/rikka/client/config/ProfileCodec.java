package sh.rikka.client.config;

import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.mojang.serialization.Codec;
import com.mojang.serialization.DataResult;
import com.mojang.serialization.JsonOps;
import java.util.List;
import org.jspecify.annotations.Nullable;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import sh.rikka.client.mod.BindMode;
import sh.rikka.client.mod.Mod;
import sh.rikka.client.setting.Setting;
import sh.rikka.client.setting.Settings;

final class ProfileCodec {
	private static final Logger LOGGER = LoggerFactory.getLogger(ProfileCodec.class);
	private static final String MODS = "mods";
	private static final String ENABLED = "enabled";
	private static final String SETTINGS = "settings";

	private ProfileCodec() {
	}

	static JsonObject encode(JsonObject base, List<Mod> mods) {
		JsonObject document = base.deepCopy();
		JsonObject modsObject = child(document, MODS);
		for (Mod mod : mods) {
			JsonObject modObject = child(modsObject, mod.id());
			modObject.addProperty(ENABLED, persistsEnabled(mod) && mod.isEnabled());
			encode(mod.controls(), modObject);
			encode(mod.settings(), child(modObject, SETTINGS));
		}
		return document;
	}

	static void apply(JsonObject document, List<Mod> mods) {
		JsonObject modsObject = object(document, MODS, MODS);
		for (Mod mod : mods) {
			JsonObject modObject = object(modsObject, mod.id(), mod.id());
			decode(mod.controls(), modObject, mod.id());
			decode(mod.settings(), object(modObject, SETTINGS, mod.id() + "." + SETTINGS), mod.id());
			boolean enabled = decode(Codec.BOOL, modObject.get(ENABLED), mod.id() + "." + ENABLED, false);
			mod.setEnabled(persistsEnabled(mod) && enabled);
		}
	}

	static boolean persistsEnabled(Mod mod) {
		return !mod.bindMode().is(BindMode.HOLD);
	}

	private static void encode(Settings settings, JsonObject target) {
		for (Setting<?, ?> setting : settings.all()) {
			target.add(setting.id(), encode(setting));
		}
	}

	private static void decode(Settings settings, JsonObject source, String path) {
		for (Setting<?, ?> setting : settings.all()) {
			decode(setting, source.get(setting.id()), path + "." + setting.id());
		}
	}

	private static JsonObject child(JsonObject parent, String key) {
		if (parent.get(key) instanceof JsonObject existing) {
			return existing;
		}
		JsonObject created = new JsonObject();
		parent.add(key, created);
		return created;
	}

	private static JsonObject object(JsonObject parent, String key, String path) {
		JsonElement element = parent.get(key);
		if (element instanceof JsonObject object) {
			return object;
		}
		if (element != null) {
			LOGGER.warn("ignoring {} in profile: expected an object", path);
		}
		return new JsonObject();
	}

	private static <T> JsonElement encode(Setting<T, ?> setting) {
		return setting.codec().encodeStart(JsonOps.INSTANCE, setting.value()).getOrThrow();
	}

	private static <T> void decode(Setting<T, ?> setting, @Nullable JsonElement element, String path) {
		setting.setValue(decode(setting.codec(), element, path, setting.defaultValue()));
	}

	private static <T> T decode(Codec<T> codec, @Nullable JsonElement element, String path, T fallback) {
		if (element == null) {
			return fallback;
		}
		DataResult<T> result = codec.parse(JsonOps.INSTANCE, element);
		return result.resultOrPartial(error -> LOGGER.warn("invalid {} in profile: {}", path, error)).orElse(fallback);
	}
}
