extends TabContainer

const PlanGraph := preload("plan_graph/plan_graph.tscn")
onready var plan_node := $"../LeftPane/PlanNode"


# when a new plan from tree is opened
func _on_Tree_open_plan(plan):
	# goto tab if already opened
	for i in get_child_count():
		if get_child(i).plan == plan:
			current_tab = i
			return
	# update plan node to new plan
	plan_node.new_plan(plan)
	# create new tab for plan
	var plan_graph := PlanGraph.instance()
	plan_graph.new_plan(plan)
	add_child(plan_graph)


func _on_CloseTabButton_pressed():
	var tab := get_child(current_tab)
	if tab:
		tab.queue_free()
