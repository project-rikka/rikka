package sh.rikka.client.hud.style;

public enum AccentPalette {
	THEME(0, 0),
	MINT(0xFFAAFFA9, 0xFF11FFBD),
	REEF(0xFF00D2FF, 0xFF3A7BD5),
	AMIN(0xFF8E2DE2, 0xFF4A00E0),
	SUBLIME(0xFFFC466B, 0xFF3F5EFB),
	TIMBER(0xFFFC00FF, 0xFF00DBDE),
	MAGIC(0xFF59C173, 0xFF5D26C1),
	SUNSET(0xFFB92B27, 0xFF1565C0),
	MANGO(0xFFF09819, 0xFFEDDE5D),
	ORANGE(0xFFFFB75E, 0xFFED8F03),
	CRIMSON(0xFFED213A, 0xFF93291E),
	BLOSSOM(0xFF800080, 0xFFFFC0CB),
	PEACH(0xFFF2709C, 0xFFFF9472),
	ABYSS(0xFF0F2027, 0xFF2C5364),
	CUSTOM(0, 0);

	private final int start;
	private final int end;

	AccentPalette(int start, int end) {
		this.start = start;
		this.end = end;
	}

	public int start() {
		return this.start;
	}

	public int end() {
		return this.end;
	}
}
