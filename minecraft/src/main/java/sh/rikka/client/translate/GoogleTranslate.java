package sh.rikka.client.translate;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonParseException;
import com.google.gson.JsonParser;
import com.google.gson.JsonPrimitive;
import java.io.IOException;
import java.io.InputStream;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.net.http.HttpTimeoutException;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.ArrayList;
import java.util.List;
import java.util.Optional;
import java.util.concurrent.Executor;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import sh.rikka.client.translate.TranslationFailure.Reason;

public final class GoogleTranslate {
	public static final int MAX_STRINGS = 100;
	public static final int MAX_CHARS = 5_000;

	private static final Logger LOGGER = LoggerFactory.getLogger(GoogleTranslate.class);
	private static final URI ENDPOINT = URI.create("https://translate-pa.googleapis.com/v1/translateHtml");
	private static final URI KEY_PAGE = URI.create(
		"https://translate.googleapis.com/_/translate_http/_/js/k=translate_http.tr.en_US.YusFYy3P_ro.O/am=AAg/d=1/exm=el_conf/ed=1/rs=AN8SPfq1Hb8iJRleQqQc8zhdzXmF9E56eQ/m=el_main"
	);
	private static final String KEY_HEADER = "x-goog-api-key";
	private static final String CONTENT_TYPE = "application/json+protobuf";
	private static final String SOURCE = "auto";
	private static final String FORMAT = "te";
	private static final Duration CONNECT_TIMEOUT = Duration.ofSeconds(10);
	private static final Duration REQUEST_TIMEOUT = Duration.ofSeconds(30);
	private static final Duration RETRY_PAUSE = Duration.ofMillis(400);
	private static final int ANSWER_LIMIT = 4 << 20;
	private static final int PAGE_LIMIT = 2 << 20;
	private static final int TOO_MANY_REQUESTS = 429;
	private static final int UNAUTHORIZED = 401;
	private static final int FORBIDDEN = 403;

	private final HttpClient client;
	private final GoogleKeys keys = new GoogleKeys();

	public GoogleTranslate(Executor executor) {
		this.client = HttpClient.newBuilder()
			.executor(executor)
			.connectTimeout(CONNECT_TIMEOUT)
			.followRedirects(HttpClient.Redirect.NORMAL)
			.build();
	}

	public List<Translated> translate(List<String> html, String target) throws TranslationFailure {
		byte[] body = body(html, Languages.serviceCode(target));
		boolean retried = false;
		while (true) {
			String key = this.keys.get(this::scrapeKey)
				.orElseThrow(() -> new TranslationFailure(Reason.KEY_UNAVAILABLE, "Google access key is unavailable"));
			HttpRequest request = HttpRequest.newBuilder(ENDPOINT)
				.timeout(REQUEST_TIMEOUT)
				.header("Content-Type", CONTENT_TYPE)
				.header(KEY_HEADER, key)
				.POST(HttpRequest.BodyPublishers.ofByteArray(body))
				.build();
			try {
				return parse(send(request, ANSWER_LIMIT), html.size());
			} catch (TranslationFailure failure) {
				if (failure.reason() != Reason.REFUSED || retried) {
					throw failure;
				}
				retried = true;
				this.keys.invalidate(key);
			}
		}
	}

	private Optional<String> scrapeKey() {
		HttpRequest request = HttpRequest.newBuilder(KEY_PAGE).timeout(REQUEST_TIMEOUT).GET().build();
		try {
			return GoogleKeys.find(send(request, PAGE_LIMIT), KEY_HEADER);
		} catch (TranslationFailure failure) {
			LOGGER.warn("failed to read Google access key page: {}", failure.getMessage());
			return Optional.empty();
		}
	}

	private String send(HttpRequest request, int limit) throws TranslationFailure {
		boolean retried = false;
		while (true) {
			HttpResponse<InputStream> response = exchange(request);
			int status = response.statusCode();
			try (InputStream stream = response.body()) {
				if (status / 100 == 2) {
					return read(stream, limit);
				}
			} catch (IOException failure) {
				throw new TranslationFailure(Reason.NETWORK, "failed to read answer: " + failure.getMessage(), failure);
			}
			if (status / 100 == 5 && !retried) {
				retried = true;
				pause();
				continue;
			}
			throw switch (status) {
				case TOO_MANY_REQUESTS -> new TranslationFailure(Reason.RATE_LIMITED, "rate limited");
				case UNAUTHORIZED, FORBIDDEN -> new TranslationFailure(Reason.REFUSED, "refused with status " + status);
				default -> new TranslationFailure(Reason.SERVICE, "unexpected status " + status);
			};
		}
	}

	private HttpResponse<InputStream> exchange(HttpRequest request) throws TranslationFailure {
		try {
			return this.client.send(request, HttpResponse.BodyHandlers.ofInputStream());
		} catch (HttpTimeoutException failure) {
			throw new TranslationFailure(Reason.TIMEOUT, "timed out", failure);
		} catch (IOException failure) {
			throw new TranslationFailure(Reason.NETWORK, "request failed: " + failure.getMessage(), failure);
		} catch (InterruptedException failure) {
			Thread.currentThread().interrupt();
			throw new TranslationFailure(Reason.NETWORK, "interrupted", failure);
		}
	}

	private static String read(InputStream stream, int limit) throws IOException, TranslationFailure {
		byte[] bytes = stream.readNBytes(limit + 1);
		if (bytes.length > limit) {
			throw new TranslationFailure(Reason.MALFORMED, "answer exceeds " + limit + " bytes");
		}
		return new String(bytes, StandardCharsets.UTF_8);
	}

	private static void pause() throws TranslationFailure {
		try {
			Thread.sleep(RETRY_PAUSE);
		} catch (InterruptedException failure) {
			Thread.currentThread().interrupt();
			throw new TranslationFailure(Reason.NETWORK, "interrupted", failure);
		}
	}

	private static byte[] body(List<String> html, String code) {
		JsonArray strings = new JsonArray(html.size());
		html.forEach(strings::add);
		JsonArray call = new JsonArray(3);
		call.add(strings);
		call.add(SOURCE);
		call.add(code);
		JsonArray root = new JsonArray(2);
		root.add(call);
		root.add(FORMAT);
		return root.toString().getBytes(StandardCharsets.UTF_8);
	}

	private static List<Translated> parse(String answer, int count) throws TranslationFailure {
		try {
			JsonArray root = JsonParser.parseString(answer).getAsJsonArray();
			JsonArray texts = root.get(0).getAsJsonArray();
			JsonArray detected = root.size() > 1 && root.get(1).isJsonArray() ? root.get(1).getAsJsonArray() : new JsonArray();
			if (texts.size() != count) {
				throw new TranslationFailure(Reason.MALFORMED, "expected " + count + " translations, got " + texts.size());
			}
			List<Translated> translated = new ArrayList<>(count);
			for (int index = 0; index < count; index++) {
				translated.add(new Translated(texts.get(index).getAsString(), detected(detected, index)));
			}
			return translated;
		} catch (JsonParseException | IllegalStateException | IndexOutOfBoundsException | UnsupportedOperationException failure) {
			throw new TranslationFailure(Reason.MALFORMED, "malformed answer: " + failure.getMessage(), failure);
		}
	}

	private static Optional<String> detected(JsonArray detected, int index) {
		if (index >= detected.size()) {
			return Optional.empty();
		}
		JsonElement element = detected.get(index);
		if (element instanceof JsonPrimitive primitive && primitive.isString() && !primitive.getAsString().isBlank()) {
			return Optional.of(Languages.fromServiceCode(primitive.getAsString()));
		}
		return Optional.empty();
	}
}
