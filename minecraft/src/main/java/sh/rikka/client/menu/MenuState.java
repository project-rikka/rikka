package sh.rikka.client.menu;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.Optional;
import os.rikka.TextDocument;
import sh.rikka.client.mod.Category;
import sh.rikka.client.mod.Mod;

public final class MenuState {
	private final List<Page> history = new ArrayList<>(List.of(new Page.Mods()));
	private final TextDocument search = new TextDocument("");
	private int position;
	private Optional<Category> category = Optional.empty();

	public Page page() {
		return this.history.get(this.position);
	}

	public void open(Page page) {
		if (page.equals(page())) {
			return;
		}
		this.history.subList(this.position + 1, this.history.size()).clear();
		this.history.add(page);
		this.position++;
	}

	public boolean canGoBack() {
		return this.position > 0;
	}

	public boolean canGoForward() {
		return this.position < this.history.size() - 1;
	}

	public void back() {
		if (canGoBack()) {
			this.position--;
		}
	}

	public void forward() {
		if (canGoForward()) {
			this.position++;
		}
	}

	public Optional<Category> category() {
		return this.category;
	}

	public void filter(Optional<Category> category) {
		this.category = category;
	}

	public TextDocument search() {
		return this.search;
	}

	public List<Mod> shown(List<Mod> mods) {
		String query = this.search.getText().strip().toLowerCase(Locale.ROOT);
		return mods.stream()
			.filter(mod -> this.category.isEmpty() || mod.category() == this.category.get())
			.filter(mod -> query.isEmpty() || matches(mod, query))
			.toList();
	}

	private static boolean matches(Mod mod, String query) {
		return mod.name().getString().toLowerCase(Locale.ROOT).contains(query)
			|| mod.description().getString().toLowerCase(Locale.ROOT).contains(query)
			|| mod.id().contains(query);
	}
}
