package sh.rikka.client.resource.legacy;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;
import java.io.IOException;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Optional;
import java.util.OptionalInt;

public record PackDescriptor(JsonObject json, int format) {
	public static final String PACK_META = "pack.mcmeta";

	private static final String PACK = "pack";
	private static final String OVERLAYS = "overlays";

	public static Optional<PackDescriptor> read(Path pack) throws IOException {
		Optional<byte[]> bytes = PackArchive.readRoot(pack, PACK_META);
		if (bytes.isEmpty() || !(Text.json(bytes.get()) instanceof JsonObject json)) {
			return Optional.empty();
		}
		OptionalInt format = format(json);
		return format.isPresent() ? Optional.of(new PackDescriptor(json, format.getAsInt())) : Optional.empty();
	}

	public static OptionalInt format(JsonObject json) {
		if (!(json.get(PACK) instanceof JsonObject pack)) {
			return OptionalInt.empty();
		}
		int format = -1;
		if (pack.get("pack_format") instanceof JsonPrimitive primitive && primitive.isNumber()) {
			format = primitive.getAsInt();
		}
		format = Math.max(format, upperBound(pack.get("supported_formats")));
		format = Math.max(format, major(pack.get("max_format")));
		return format < 0 ? OptionalInt.empty() : OptionalInt.of(format);
	}

	public List<String> overlays() {
		List<String> directories = new ArrayList<>();
		if (!(this.json.get(OVERLAYS) instanceof JsonObject overlays) || !(overlays.get("entries") instanceof JsonArray entries)) {
			return directories;
		}
		for (JsonElement element : entries) {
			if (!(element instanceof JsonObject entry) || !(entry.get("directory") instanceof JsonPrimitive directory)) {
				continue;
			}
			int min = Math.max(lowerBound(entry.get("formats")), major(entry.get("min_format")));
			int max = Math.max(upperBound(entry.get("formats")), major(entry.get("max_format")));
			if (min >= 0 && this.format >= min && (max < 0 || this.format <= max)) {
				directories.add(directory.getAsString());
			}
		}
		return directories;
	}

	private static int major(JsonElement element) {
		if (element instanceof JsonPrimitive primitive && primitive.isNumber()) {
			return primitive.getAsInt();
		}
		if (element instanceof JsonArray array && !array.isEmpty()) {
			return array.get(0).getAsInt();
		}
		return -1;
	}

	private static int lowerBound(JsonElement element) {
		if (element instanceof JsonPrimitive primitive && primitive.isNumber()) {
			return primitive.getAsInt();
		}
		if (element instanceof JsonArray array && array.size() == 2) {
			return array.get(0).getAsInt();
		}
		if (element instanceof JsonObject object && object.get("min_inclusive") instanceof JsonPrimitive min) {
			return min.getAsInt();
		}
		return -1;
	}

	private static int upperBound(JsonElement element) {
		if (element instanceof JsonPrimitive primitive && primitive.isNumber()) {
			return primitive.getAsInt();
		}
		if (element instanceof JsonArray array && array.size() == 2) {
			return array.get(1).getAsInt();
		}
		if (element instanceof JsonObject object && object.get("max_inclusive") instanceof JsonPrimitive max) {
			return max.getAsInt();
		}
		return -1;
	}
}
