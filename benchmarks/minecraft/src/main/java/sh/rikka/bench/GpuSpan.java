package sh.rikka.bench;

public enum GpuSpan {
	FRAME("frame"),
	SKY("sky"),
	SOLID("solid"),
	TRANSPARENT("transparent"),
	GUI("gui");

	private final String label;

	GpuSpan(String label) {
		this.label = label;
	}

	public String label() {
		return this.label;
	}
}
