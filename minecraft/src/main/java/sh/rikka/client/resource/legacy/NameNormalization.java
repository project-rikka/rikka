package sh.rikka.client.resource.legacy;

import java.util.Locale;
import java.util.Optional;

final class NameNormalization implements Upgrade {
	static String normalize(String path) {
		String lower = path.toLowerCase(Locale.ROOT);
		StringBuilder result = new StringBuilder(lower.length());
		for (int i = 0; i < lower.length(); i++) {
			char c = lower.charAt(i);
			boolean valid = c >= 'a' && c <= 'z' || c >= '0' && c <= '9' || c == '/' || c == '.' || c == '_' || c == '-';
			result.append(valid ? c : '_');
		}
		return result.toString();
	}

	@Override
	public void apply(Conversion conversion) {
		Assets assets = conversion.assets();
		for (String path : assets.paths(Assets.ROOT)) {
			String normalized = normalize(path);
			if (!normalized.equals(path) && !assets.has(normalized)) {
				assets.move(path, normalized);
			}
		}
		Documents documents = conversion.documents();
		for (String path : documents.paths(Assets.ROOT)) {
			String normalized = normalize(path);
			if (!normalized.equals(path) && !documents.has(normalized)) {
				documents.put(normalized, documents.remove(path).orElseThrow());
			}
		}
		Renaming.rewriteTextureReferences(conversion, id -> Optional.of(normalize(id)));
		Renaming.rewriteModelReferences(conversion, id -> Optional.of(normalize(id)));
	}
}
