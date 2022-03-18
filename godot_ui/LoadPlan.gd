extends ConfirmationDialog

signal success(plan)
signal error(msg)

# Called when the node enters the scene tree for the first time.
func _ready():
	pass

func _on_MenuButton_id_pressed(id):
	if id == 1:
		show()
		
func verify(plan):
	if not plan.has("name"):
		emit_signal("error", "Plan has no name.")
		return false
	if not plan.has("behaviour"):
		emit_signal("error", plan["name"] + " plan has no behaviour.")
		return false
	if not plan.has("transitions"):
		emit_signal("error", plan["name"] + " plan has no transitions vector.")
		return false
	if not plan.has("plans"):
		emit_signal("error", plan["name"] + " plan has no plans vector.")
		return false
	for child in plan["plans"]:
		if not verify(child):
			return false
	return true

func _confirmed():
	var text : String = get_node("TextEdit").text
	var plan = JSON.parse(text).result
	
	if not plan:
		emit_signal("error", "Could not parse JSON.")
	elif verify(plan):
		Global.plan_tree = plan
		emit_signal("success", plan)
		hide()
