package sh.rikka.client.mods.visual;

public enum ChunkGrid {
	NONE(0),
	SPARSE(4),
	NORMAL(2),
	DENSE(1);

	private final int spacing;

	ChunkGrid(int spacing) {
		this.spacing = spacing;
	}

	int spacing() {
		return this.spacing;
	}
}
