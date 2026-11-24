package sh.rikka.client.resource.legacy;

final class Conversion {
	private final Assets assets;
	private final Documents documents;
	private final int sourceFormat;
	private final Vanilla vanilla;

	public Conversion(Assets assets, Documents documents, int sourceFormat, Vanilla vanilla) {
		this.assets = assets;
		this.documents = documents;
		this.sourceFormat = sourceFormat;
		this.vanilla = vanilla;
	}

	public Assets assets() {
		return this.assets;
	}

	public Documents documents() {
		return this.documents;
	}

	public int sourceFormat() {
		return this.sourceFormat;
	}

	public Vanilla vanilla() {
		return this.vanilla;
	}

	public boolean sourceBefore(int format) {
		return this.sourceFormat < format;
	}
}
