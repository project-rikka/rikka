package sh.rikka.client.mods.camera;

import net.minecraft.client.CameraType;

public enum Viewpoint {
	BEHIND(CameraType.THIRD_PERSON_BACK),
	FRONT(CameraType.THIRD_PERSON_FRONT);

	private final CameraType cameraType;

	Viewpoint(CameraType cameraType) {
		this.cameraType = cameraType;
	}

	CameraType cameraType() {
		return this.cameraType;
	}
}
