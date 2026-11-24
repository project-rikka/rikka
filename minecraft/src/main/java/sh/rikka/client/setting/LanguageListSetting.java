package sh.rikka.client.setting;

import com.mojang.serialization.Codec;
import com.mojang.serialization.DataResult;
import java.util.ArrayList;
import java.util.LinkedHashSet;
import java.util.List;
import sh.rikka.client.translate.Languages;

public final class LanguageListSetting extends Setting<List<String>, LanguageListSetting> {
	private static final Codec<List<String>> CODEC = Codec.STRING.listOf().flatXmap(LanguageListSetting::validated, DataResult::success);

	private final List<String> defaultValue;
	private volatile List<String> value;

	LanguageListSetting(String id, String translationKey, List<String> defaultValue) {
		super(id, translationKey);
		for (String tag : defaultValue) {
			if (!Languages.isTarget(tag)) {
				throw new IllegalArgumentException("default of " + id + " holds an unknown language: " + tag);
			}
		}
		this.defaultValue = distinct(defaultValue);
		this.value = this.defaultValue;
	}

	private static DataResult<List<String>> validated(List<String> tags) {
		List<String> known = distinct(tags.stream().filter(Languages::isTarget).toList());
		if (known.size() == tags.size()) {
			return DataResult.success(known);
		}
		return DataResult.error(() -> "unknown or repeated languages in " + tags, known);
	}

	private static List<String> distinct(List<String> tags) {
		return List.copyOf(new LinkedHashSet<>(tags));
	}

	public List<String> get() {
		return this.value;
	}

	public void set(List<String> tags) {
		for (String tag : tags) {
			if (!Languages.isTarget(tag)) {
				throw new IllegalArgumentException(id() + " does not offer language " + tag);
			}
		}
		List<String> copied = distinct(tags);
		if (this.value.equals(copied)) {
			return;
		}
		this.value = copied;
		changed(copied);
	}

	public void add(String tag) {
		List<String> next = new ArrayList<>(this.value);
		next.add(tag);
		set(next);
	}

	public void remove(String tag) {
		List<String> next = new ArrayList<>(this.value);
		if (next.remove(tag)) {
			set(next);
		}
	}

	@Override
	public List<String> value() {
		return this.value;
	}

	@Override
	public List<String> defaultValue() {
		return this.defaultValue;
	}

	@Override
	public void setValue(List<String> value) {
		set(value);
	}

	@Override
	public Codec<List<String>> codec() {
		return CODEC;
	}
}
