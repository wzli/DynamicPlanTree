extends ConfirmationDialog

onready var text: String = $TextEdit.text


func _ready():
	add_button("Reset", false, "reset")
	reset()


func _on_MenuButton_id_pressed(id):
	if id == 0:
		show()


func _confirmed():
	var parsed = JSON.parse($TextEdit.text)
	var schema = parsed.result
	if parsed.error:
		Global.error_msg(parsed.error_string + " at line " + String(parsed.error_line))
	elif not schema.has("BehaviourEnum"):
		Global.error_msg("Missing behaviours schema.")
	elif not schema.has("PredicateEnum"):
		Global.error_msg("Missing predicates schema.")
	else:
		Global.update_schema(schema)
		hide()


func _custom_action(action: String):
	if action == "reset":
		reset()


func reset():
	$TextEdit.text = JSON.print(Global.schema, "  ")
