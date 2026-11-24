package sh.rikka.client.mods.motionblur;

import org.joml.Matrix4fc;
import org.joml.Vector3fc;

record CameraStep(
	Matrix4fc viewInverse,
	Matrix4fc projectionInverse,
	Matrix4fc previousView,
	Matrix4fc previousProjection,
	Vector3fc delta,
	boolean cut
) {
}
