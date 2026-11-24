package sh.rikka.client.resource.legacy;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonElement;
import com.google.gson.JsonParseException;
import com.google.gson.JsonParser;
import com.google.gson.Strictness;
import com.google.gson.stream.JsonReader;
import java.io.IOException;
import java.io.StringReader;
import java.nio.charset.StandardCharsets;

final class Text {
	private static final char BYTE_ORDER_MARK = '\uFEFF';
	private static final Gson GSON = new GsonBuilder().disableHtmlEscaping().create();

	private Text() {
	}

	public static String decode(byte[] bytes) {
		String text = new String(bytes, StandardCharsets.UTF_8);
		return !text.isEmpty() && text.charAt(0) == BYTE_ORDER_MARK ? text.substring(1) : text;
	}

	public static JsonElement json(byte[] bytes) throws IOException {
		JsonReader reader = new JsonReader(new StringReader(decode(bytes)));
		reader.setStrictness(Strictness.LENIENT);
		try {
			return JsonParser.parseReader(reader);
		} catch (JsonParseException failure) {
			throw new IOException("malformed JSON: " + failure.getMessage(), failure);
		}
	}

	public static byte[] encode(JsonElement json) {
		return GSON.toJson(json).getBytes(StandardCharsets.UTF_8);
	}
}
