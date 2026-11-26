package sh.rikka.bench;

public record Flight(String name, int ticks, Path path) {
	@FunctionalInterface
	public interface Path {
		Pose at(int tick);
	}

	public static Flight hover(String name, int ticks, Pose pose) {
		return new Flight(name, ticks, tick -> pose);
	}

	public static Flight spin(String name, int ticks, Pose pose) {
		return new Flight(name, ticks, tick -> new Pose(pose.x(), pose.y(), pose.z(), pose.yaw() + 360.0F * tick / ticks, pose.pitch()));
	}

	public static Flight line(String name, int ticks, Pose start, double blocksPerTick) {
		double radians = Math.toRadians(start.yaw());
		double stepX = -Math.sin(radians) * blocksPerTick;
		double stepZ = Math.cos(radians) * blocksPerTick;
		return new Flight(name, ticks, tick -> new Pose(start.x() + stepX * tick, start.y(), start.z() + stepZ * tick, start.yaw(), start.pitch()));
	}

	public static Flight orbit(String name, int ticks, double centerX, double y, double centerZ, double radius, float pitch) {
		return new Flight(name, ticks, tick -> {
			double angle = 2.0 * Math.PI * tick / ticks;
			double x = centerX + Math.cos(angle) * radius;
			double z = centerZ + Math.sin(angle) * radius;
			return new Pose(x, y, z, (float) Math.toDegrees(angle), pitch);
		});
	}
}
