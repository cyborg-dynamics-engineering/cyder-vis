extends SceneTree


func _initialize() -> void:
	assert(ConversionCalculator.convert_f32("0.001") == "6f 12 83 3a")
	assert(ConversionCalculator.convert_f32("0.001", true) == "3a 83 12 6f")
	assert(ConversionCalculator.convert_f32("-1") == "00 00 80 bf")
	assert(ConversionCalculator.convert_f32("-0") == "00 00 00 80")
	assert(ConversionCalculator.convert_f32("1e0", true) == "3f 80 00 00")
	for invalid in ["", "abc", "nan", "inf", "3.5e38", "1.2.3"]:
		assert(ConversionCalculator.convert_f32(invalid).is_empty())
	print("Float32 conversion tests passed")
	quit()
