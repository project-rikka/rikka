package sh.rikka.client.resource.legacy;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

final class ItemDisplay implements Upgrade {
	private static final String FIRST_PERSON = "firstperson";
	private static final String THIRD_PERSON = "thirdperson";
	private static final String BUILTIN_GENERATED = "builtin/generated";
	private static final String ITEM_GENERATED = "item/generated";

	private static final double[] FIRST_PERSON_ROTATION = {0, 45, 0};
	private static final double[] FIRST_PERSON_PIVOT = {0, 4, 2};
	private static final double FIRST_PERSON_SCALE = 0.4;
	private static final double[] FIRST_PERSON_OFFSET = {1.13, 3.2, 1.13};
	private static final double[] THIRD_PERSON_ROTATION_SIGN = {1, -1, -1};
	private static final double[] THIRD_PERSON_ROTATION = {0, 0, 20};
	private static final double[] THIRD_PERSON_TRANSLATION_SIGN = {1, 1, -1};
	private static final double[] THIRD_PERSON_TRANSLATION = {0, 2.75, -3};

	@Override
	public void apply(Conversion conversion) {
		Documents documents = conversion.documents();
		for (String path : documents.paths(Renaming::isModel)) {
			if (!(documents.get(path).orElseThrow() instanceof JsonObject model) || !(model.get("display") instanceof JsonObject display)) {
				continue;
			}
			if (!display.has(FIRST_PERSON) && !display.has(THIRD_PERSON)) {
				continue;
			}
			JsonElement first = display.remove(FIRST_PERSON);
			JsonElement third = display.remove(THIRD_PERSON);
			if (!isItemModel(path, model)) {
				continue;
			}
			if (first instanceof JsonObject transform) {
				JsonObject right = firstPerson(transform);
				display.add("firstperson_righthand", right);
				display.add("firstperson_lefthand", leftHand(right));
			}
			if (third instanceof JsonObject transform) {
				JsonObject right = thirdPerson(transform);
				display.add("thirdperson_righthand", right);
				display.add("thirdperson_lefthand", leftHand(right));
			}
			if (model.get("parent") instanceof JsonPrimitive parent && Renaming.unqualified(parent.getAsString()).equals(BUILTIN_GENERATED)) {
				model.addProperty("parent", ITEM_GENERATED);
			}
		}
	}

	private static boolean isItemModel(String path, JsonObject model) {
		if (!(model.get("parent") instanceof JsonPrimitive parent)) {
			return path.startsWith(Renaming.ITEM_MODELS);
		}
		String id = Renaming.unqualified(parent.getAsString());
		return id.equals(BUILTIN_GENERATED) || id.startsWith("item") || path.startsWith(Renaming.ITEM_MODELS) && !id.startsWith("block");
	}

	private static JsonObject firstPerson(JsonObject old) {
		JsonObject result = old.deepCopy();
		map(result, "rotation", value -> add(value, FIRST_PERSON_ROTATION));
		map(result, "translation", value -> add(scale(subtract(value, FIRST_PERSON_PIVOT), FIRST_PERSON_SCALE), FIRST_PERSON_OFFSET));
		map(result, "scale", value -> scale(value, FIRST_PERSON_SCALE));
		return result;
	}

	private static JsonObject thirdPerson(JsonObject old) {
		JsonObject result = old.deepCopy();
		map(result, "rotation", value -> add(multiply(value, THIRD_PERSON_ROTATION_SIGN), THIRD_PERSON_ROTATION));
		map(result, "translation", value -> add(multiply(value, THIRD_PERSON_TRANSLATION_SIGN), THIRD_PERSON_TRANSLATION));
		return result;
	}

	private static JsonObject leftHand(JsonObject right) {
		JsonObject result = right.deepCopy();
		map(result, "rotation", value -> multiply(value, new double[] {1, -1, -1}));
		return result;
	}

	private interface Vector3 {
		double[] apply(double[] value);
	}

	private static void map(JsonObject transform, String key, Vector3 function) {
		if (!(transform.get(key) instanceof JsonArray array) || array.size() != 3) {
			return;
		}
		double[] value = new double[3];
		for (int i = 0; i < 3; i++) {
			if (!(array.get(i) instanceof JsonPrimitive component) || !component.isNumber()) {
				return;
			}
			value[i] = component.getAsDouble();
		}
		double[] mapped = function.apply(value);
		JsonArray result = new JsonArray();
		for (double component : mapped) {
			result.add(Math.round(component * 10000.0) / 10000.0);
		}
		transform.add(key, result);
	}

	private static double[] add(double[] a, double[] b) {
		return new double[] {a[0] + b[0], a[1] + b[1], a[2] + b[2]};
	}

	private static double[] subtract(double[] a, double[] b) {
		return new double[] {a[0] - b[0], a[1] - b[1], a[2] - b[2]};
	}

	private static double[] multiply(double[] a, double[] b) {
		return new double[] {a[0] * b[0], a[1] * b[1], a[2] * b[2]};
	}

	private static double[] scale(double[] a, double factor) {
		return new double[] {a[0] * factor, a[1] * factor, a[2] * factor};
	}
}
