package sh.rikka.bench;

public record Pose(double x, double y, double z, float yaw, float pitch) {
	public Pose raised(double height) {
		return new Pose(this.x, this.y + height, this.z, this.yaw, this.pitch);
	}

	public Pose moved(double distance) {
		return new Pose(this.x + distance, this.y, this.z + distance, this.yaw, this.pitch);
	}
}
