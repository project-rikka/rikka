use serde_json::{Value, json};

fn vector3() -> Value {
    json!({
        "type": "object",
        "properties": {
            "x": { "type": "number" },
            "y": { "type": "number" },
            "z": { "type": "number" }
        }
    })
}

fn degree_map() -> Value {
    json!({
        "type": "object",
        "properties": {
            "curve": { "type": "array", "items": { "type": "number" } },
            "xRange": { "type": "number" },
            "yRange": { "type": "number" }
        }
    })
}

fn meta() -> Value {
    json!({
        "type": "object",
        "properties": {
            "title": { "type": "string" },
            "version": { "type": "string" },
            "author": { "type": "string" },
            "contactInformation": { "type": "string" },
            "reference": { "type": "string" },
            "texture": { "type": "integer" },
            "allowedUserName": {
                "type": "string",
                "enum": ["OnlyAuthor", "ExplicitlyLicensedPerson", "Everyone"]
            },
            "violentUssageName": { "type": "string", "enum": ["Disallow", "Allow"] },
            "sexualUssageName": { "type": "string", "enum": ["Disallow", "Allow"] },
            "commercialUssageName": { "type": "string", "enum": ["Disallow", "Allow"] },
            "otherPermissionUrl": { "type": "string" },
            "licenseName": {
                "type": "string",
                "enum": [
                    "Redistribution_Prohibited", "CC0", "CC_BY", "CC_BY_NC", "CC_BY_SA",
                    "CC_BY_NC_SA", "CC_BY_ND", "CC_BY_NC_ND", "Other"
                ]
            },
            "otherLicenseUrl": { "type": "string" }
        }
    })
}

fn humanoid() -> Value {
    json!({
        "type": "object",
        "properties": {
            "humanBones": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "bone": {
                            "type": "string",
                            "enum": [
                                "hips", "leftUpperLeg", "rightUpperLeg", "leftLowerLeg",
                                "rightLowerLeg", "leftFoot", "rightFoot", "spine", "chest",
                                "neck", "head", "leftShoulder", "rightShoulder", "leftUpperArm",
                                "rightUpperArm", "leftLowerArm", "rightLowerArm", "leftHand",
                                "rightHand", "leftToes", "rightToes", "leftEye", "rightEye",
                                "jaw", "leftThumbProximal", "leftThumbIntermediate",
                                "leftThumbDistal", "leftIndexProximal", "leftIndexIntermediate",
                                "leftIndexDistal", "leftMiddleProximal",
                                "leftMiddleIntermediate", "leftMiddleDistal", "leftRingProximal",
                                "leftRingIntermediate", "leftRingDistal", "leftLittleProximal",
                                "leftLittleIntermediate", "leftLittleDistal",
                                "rightThumbProximal", "rightThumbIntermediate",
                                "rightThumbDistal", "rightIndexProximal",
                                "rightIndexIntermediate", "rightIndexDistal",
                                "rightMiddleProximal", "rightMiddleIntermediate",
                                "rightMiddleDistal", "rightRingProximal", "rightRingIntermediate",
                                "rightRingDistal", "rightLittleProximal",
                                "rightLittleIntermediate", "rightLittleDistal", "upperChest"
                            ]
                        },
                        "node": { "type": "integer" },
                        "useDefaultValues": { "type": "boolean" },
                        "min": vector3(),
                        "max": vector3(),
                        "center": vector3(),
                        "axisLength": { "type": "number" }
                    }
                }
            },
            "armStretch": { "type": "number" },
            "legStretch": { "type": "number" },
            "upperArmTwist": { "type": "number" },
            "lowerArmTwist": { "type": "number" },
            "upperLegTwist": { "type": "number" },
            "lowerLegTwist": { "type": "number" },
            "feetSpacing": { "type": "number" },
            "hasTranslationDoF": { "type": "boolean" }
        }
    })
}

fn first_person() -> Value {
    json!({
        "type": "object",
        "properties": {
            "firstPersonBone": { "type": "integer" },
            "firstPersonBoneOffset": vector3(),
            "meshAnnotations": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "mesh": { "type": "integer" },
                        "firstPersonFlag": { "type": "string" }
                    }
                }
            },
            "lookAtTypeName": { "type": "string", "enum": ["Bone", "BlendShape"] },
            "lookAtHorizontalInner": degree_map(),
            "lookAtHorizontalOuter": degree_map(),
            "lookAtVerticalDown": degree_map(),
            "lookAtVerticalUp": degree_map()
        }
    })
}

fn blend_shape() -> Value {
    json!({
        "type": "object",
        "properties": {
            "blendShapeGroups": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" },
                        "presetName": {
                            "type": "string",
                            "enum": [
                                "unknown", "neutral", "a", "i", "u", "e", "o", "blink", "joy",
                                "angry", "sorrow", "fun", "lookup", "lookdown", "lookleft",
                                "lookright", "blink_l", "blink_r"
                            ]
                        },
                        "binds": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "mesh": { "type": "integer", "minimum": 0 },
                                    "index": { "type": "integer", "minimum": 0 },
                                    "weight": { "type": "number", "minimum": 0, "maximum": 100 }
                                }
                            }
                        },
                        "materialValues": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "materialName": { "type": "string" },
                                    "propertyName": { "type": "string" },
                                    "targetValue": { "type": "array", "items": { "type": "number" } }
                                }
                            }
                        },
                        "isBinary": { "type": "boolean" }
                    }
                }
            }
        }
    })
}

fn secondary_animation() -> Value {
    json!({
        "type": "object",
        "properties": {
            "boneGroups": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "comment": { "type": "string" },
                        "stiffiness": { "type": "number" },
                        "gravityPower": { "type": "number" },
                        "gravityDir": vector3(),
                        "dragForce": { "type": "number" },
                        "center": { "type": "integer" },
                        "hitRadius": { "type": "number" },
                        "bones": { "type": "array", "items": { "type": "integer" } },
                        "colliderGroups": { "type": "array", "items": { "type": "integer" } }
                    }
                }
            },
            "colliderGroups": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "node": { "type": "integer" },
                        "colliders": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "offset": vector3(),
                                    "radius": { "type": "number" }
                                }
                            }
                        }
                    }
                }
            }
        }
    })
}

fn material() -> Value {
    json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "shader": { "type": "string" },
            "renderQueue": { "type": "integer" },
            "floatProperties": { "type": "object", "additionalProperties": { "type": "number" } },
            "vectorProperties": {
                "type": "object",
                "additionalProperties": { "type": "array", "items": { "type": "number" } }
            },
            "textureProperties": {
                "type": "object",
                "additionalProperties": { "type": "integer" }
            },
            "keywordMap": { "type": "object", "additionalProperties": { "type": "boolean" } },
            "tagMap": { "type": "object", "additionalProperties": { "type": "string" } }
        }
    })
}

pub(crate) fn vrm0() -> Value {
    json!({
        "type": "object",
        "properties": {
            "exporterVersion": { "type": "string" },
            "specVersion": { "type": "string" },
            "meta": meta(),
            "humanoid": humanoid(),
            "firstPerson": first_person(),
            "blendShapeMaster": blend_shape(),
            "secondaryAnimation": secondary_animation(),
            "materialProperties": { "type": "array", "items": material() }
        }
    })
}

pub(crate) fn violations(value: &Value, schema: &Value) -> Vec<String> {
    let mut found = Vec::new();
    check(value, schema, "VRM", &mut found);
    found
}

fn check(value: &Value, schema: &Value, path: &str, found: &mut Vec<String>) {
    if let Some(kind) = schema["type"].as_str() {
        let matches = match kind {
            "object" => value.is_object(),
            "array" => value.is_array(),
            "string" => value.is_string(),
            "boolean" => value.is_boolean(),
            "number" => value.is_number(),
            "integer" => value.is_i64() || value.is_u64(),
            _ => false,
        };
        if !matches {
            found.push(format!("{path}: expected {kind}, found {value}"));
            return;
        }
    }
    if let Some(allowed) = schema["enum"].as_array()
        && !allowed.contains(value)
    {
        found.push(format!("{path}: {value} is not one of {allowed:?}"));
    }
    if let (Some(minimum), Some(number)) = (schema["minimum"].as_f64(), value.as_f64())
        && number < minimum
    {
        found.push(format!("{path}: {number} is below {minimum}"));
    }
    if let (Some(maximum), Some(number)) = (schema["maximum"].as_f64(), value.as_f64())
        && number > maximum
    {
        found.push(format!("{path}: {number} is above {maximum}"));
    }
    if let Some(object) = value.as_object() {
        let properties = schema["properties"].as_object();
        for (key, member) in object {
            let member_path = format!("{path}.{key}");
            match properties.and_then(|properties| properties.get(key)) {
                Some(property) => check(member, property, &member_path, found),
                None if schema.get("additionalProperties").is_some() => {
                    check(member, &schema["additionalProperties"], &member_path, found)
                }
                None => {}
            }
        }
    }
    if let (Some(items), Some(array)) = (schema.get("items"), value.as_array()) {
        for (index, item) in array.iter().enumerate() {
            check(item, items, &format!("{path}[{index}]"), found);
        }
    }
}
