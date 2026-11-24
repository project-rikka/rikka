package sh.rikka.client.resource.legacy;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import java.io.IOException;
import java.io.InputStream;
import java.io.UncheckedIOException;
import java.util.ArrayList;
import java.util.Collections;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.NavigableMap;
import java.util.TreeMap;

final class LegacyData {
	private static final String RENAMES = "renames.json";
	private static final String SHEETS = "sheets.json";
	private static final String LANG_KEYS = "lang.json";

	private static final List<RenameStep> RENAME_STEPS = loadRenames();
	private static final List<SheetGroup> SHEET_GROUPS = loadSheets();
	private static final Map<String, String> LANG_KEY_RENAMES = loadLangKeys();

	private LegacyData() {
	}

	static RenameStep renameStep(int before) {
		return RENAME_STEPS.stream()
			.filter(step -> step.before() == before)
			.findFirst()
			.orElseThrow(() -> new IllegalStateException("no legacy renames before format " + before));
	}

	public static List<SheetGroup> sheetGroups() {
		return SHEET_GROUPS;
	}

	public static Map<String, String> langKeyRenames() {
		return LANG_KEY_RENAMES;
	}

	private static Map<String, String> loadLangKeys() {
		Map<String, String> keys = new HashMap<>();
		for (Map.Entry<String, JsonElement> entry : read(LANG_KEYS).getAsJsonObject().entrySet()) {
			keys.put(entry.getKey(), entry.getValue().getAsString());
		}
		return Map.copyOf(keys);
	}

	private static JsonElement read(String name) {
		try (InputStream in = LegacyData.class.getResourceAsStream(name)) {
			if (in == null) {
				throw new IllegalStateException("missing legacy pack data " + name);
			}
			return Text.json(in.readAllBytes());
		} catch (IOException failure) {
			throw new UncheckedIOException("failed to read legacy pack data " + name, failure);
		}
	}

	private static List<RenameStep> loadRenames() {
		List<RenameStep> steps = new ArrayList<>();
		for (JsonElement element : read(RENAMES).getAsJsonArray()) {
			JsonObject step = element.getAsJsonObject();
			steps.add(new RenameStep(step.get("before").getAsInt(), renames(step.getAsJsonObject("textures")), renames(step.getAsJsonObject("models"))));
		}
		return List.copyOf(steps);
	}

	private static Map<String, List<String>> renames(JsonObject table) {
		Map<String, List<String>> renames = new LinkedHashMap<>();
		for (Map.Entry<String, JsonElement> entry : table.entrySet()) {
			List<String> targets = new ArrayList<>();
			entry.getValue().getAsJsonArray().forEach(target -> targets.add(target.getAsString()));
			renames.put(entry.getKey(), List.copyOf(targets));
		}
		return Collections.unmodifiableMap(renames);
	}

	private static List<SheetGroup> loadSheets() {
		List<SheetGroup> groups = new ArrayList<>();
		for (JsonElement element : read(SHEETS).getAsJsonArray()) {
			JsonObject group = element.getAsJsonObject();
			NavigableMap<Integer, Integer> widths = new TreeMap<>();
			group.getAsJsonObject("widths").entrySet().forEach(entry -> widths.put(Integer.parseInt(entry.getKey()), entry.getValue().getAsInt()));
			List<SheetSprite> sprites = new ArrayList<>();
			for (JsonElement spriteElement : group.getAsJsonArray("sprites")) {
				JsonObject sprite = spriteElement.getAsJsonObject();
				int since = sprite.has("since") ? sprite.get("since").getAsInt() : PackFormats.V1_6;
				String id = sprite.get("sprite").getAsString();
				if (sprite.has("rect")) {
					JsonArray rect = sprite.getAsJsonArray("rect");
					sprites.add(new SheetSprite(id, Rect.of(rect.get(0).getAsInt(), rect.get(1).getAsInt(), rect.get(2).getAsInt(), rect.get(3).getAsInt()), -1, 0, since));
				} else {
					sprites.add(new SheetSprite(id, null, sprite.get("index").getAsInt(), sprite.get("cell").getAsInt(), since));
				}
			}
			groups.add(new SheetGroup(
				group.get("sheet").getAsString(),
				group.get("before").getAsInt(),
				widths,
				group.has("retire") && group.get("retire").getAsBoolean(),
				List.copyOf(sprites)
			));
		}
		return List.copyOf(groups);
	}

	public record RenameStep(int before, Map<String, List<String>> textures, Map<String, List<String>> models) {
	}

	public record SheetGroup(String sheet, int before, NavigableMap<Integer, Integer> widths, boolean retire, List<SheetSprite> sprites) {
		public int referenceWidth(int sourceFormat) {
			Map.Entry<Integer, Integer> entry = this.widths.floorEntry(sourceFormat);
			return entry != null ? entry.getValue() : this.widths.firstEntry().getValue();
		}
	}

	public record SheetSprite(String sprite, Rect rect, int index, int cell, int since) {
		public Rect rect(int referenceWidth) {
			if (this.rect != null) {
				return this.rect;
			}
			int columns = referenceWidth / this.cell;
			return Rect.of(this.index % columns * this.cell, this.index / columns * this.cell, this.cell, this.cell);
		}
	}
}
