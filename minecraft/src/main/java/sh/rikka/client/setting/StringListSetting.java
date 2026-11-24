package sh.rikka.client.setting;

import com.mojang.serialization.Codec;
import java.util.ArrayList;
import java.util.List;

public final class StringListSetting extends Setting<List<String>, StringListSetting> {
	private static final Codec<List<String>> CODEC = Codec.STRING.listOf().xmap(List::copyOf, List::copyOf);

	private final List<String> defaultValue;
	private volatile List<String> value;

	StringListSetting(String id, String translationKey, List<String> defaultValue) {
		super(id, translationKey);
		this.defaultValue = List.copyOf(defaultValue);
		this.value = this.defaultValue;
	}

	public List<String> get() {
		return this.value;
	}

	public boolean contains(String entry) {
		return this.value.contains(entry);
	}

	public void set(List<String> value) {
		List<String> copied = List.copyOf(value);
		if (this.value.equals(copied)) {
			return;
		}
		this.value = copied;
		changed(copied);
	}

	public void add(String entry) {
		List<String> next = new ArrayList<>(this.value);
		next.add(entry);
		set(next);
	}

	public void remove(String entry) {
		List<String> next = new ArrayList<>(this.value);
		if (next.remove(entry)) {
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
