extends Tree

signal open_plan(plan)

# Declare member variables here. Examples:
# var a = 2
# var b = "text"

# Called when the node enters the scene tree for the first time.
func _ready():
	create_tree(create_item(), Global.plan_tree)

func create_tree(root, plan):
	root.set_text(0, plan["name"])
	for sub_plan in plan["plans"]:
		create_tree(create_item(root), sub_plan)

func _item_activated():
	var path = []
	var node = get_selected()
	while node :
		path.push_back(node.get_text(0))
		node = node.get_parent()
	var plan = Global.plan_tree
	path.pop_back()
	path.invert()
	for name in path:
		for sub_plan in plan["plans"]:
			if sub_plan["name"] == name:
				plan = sub_plan
	emit_signal("open_plan", plan)


func _on_LoadPlan_success(plan):
	clear()
	var tabs = get_node("../../TabContainer")
	if tabs.get_child_count():
		for tab in tabs.get_children():
			tabs.remove_child(tab) 
		
	create_tree(create_item(), plan)
