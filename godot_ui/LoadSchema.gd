extends ConfirmationDialog

signal error(msg)

# Called when the node enters the scene tree for the first time.
func _ready():
	pass # Replace with function body.

func _on_MenuButton_id_pressed(id):
	if id == 0:
		show()

func _confirmed():
	var text : String = get_node("TextEdit").text
	var schema = JSON.parse(text).result
	
	if not schema :
		emit_signal("error", "Could not parse JSON.")
	elif not schema.has("BehaviourEnum"):
		emit_signal("error",  "Missing behaviours schema.")
	elif not schema.has("PredicateEnum"):
		emit_signal("error", "Missing predicates schema.")
	else:
		Global.schema = schema
		Global.schema_version = hash(schema)
		print("new schema " + String(Global.schema_version))
		hide()
