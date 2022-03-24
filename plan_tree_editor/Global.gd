extends Node

var plan_tree: Dictionary
var schema: Dictionary


func _ready():
	plan_tree = load_from_user_or_res("plan_tree.json")
	schema = load_from_user_or_res("schema.json")


func error_msg(msg: String):
	get_tree().call_group("error_msg", "error_msg", msg)


func update_plan_tree(update: Dictionary):
	plan_tree = update
	save_json(plan_tree, "user://plan_tree.json")
	get_tree().call_group("update_plan_tree", "update_plan_tree")


func update_schema(update: Dictionary):
	schema = update
	save_json(schema, "user://schema.json")
	get_tree().call_group("update_schema", "update_schema")


func save_json(dict: Dictionary, path: String):
	var json_file := File.new()
	json_file.open(path, File.WRITE)
	json_file.store_string(JSON.print(dict, "  "))
	json_file.close()


func load_from_user_or_res(path: String) -> Dictionary:
	var dict = load_json_file("user://" + path)
	if not dict:
		dict = load_json_file("res://" + path)
	assert(dict)
	return dict


func load_json_file(path: String):
	var json_file := File.new()
	if not json_file.file_exists(path):
		return
	json_file.open(path, File.READ)
	var parsed := JSON.parse(json_file.get_as_text())
	if parsed.error:
		print("load_json_file: " + parsed.error_string + " at line " + String(parsed.error_line))
		return
	return parsed.result
