tool bash {"command": "cat other.txt"}
tool write {"path": "other.txt", "content": "mine\n"}
tool read {"path": "f.txt"}
tool edit {"path": "f.txt", "old": "one", "new": "ONE"} tool edit {"path": "f.txt", "old": "two", "new": "TWO"}
tool edit {"path": "f.txt", "old": "ONE", "new": "1"} tool bash {"command": "sed -i s/TWO/2/ f.txt"} tool edit {"path": "f.txt", "old": "1", "new": "uno"}
tool bash {"command": "echo a > new.txt"}
tool write {"path": "new.txt", "content": "b\n"}
tool bash {"command": "cat f.txt other.txt new.txt"}
tool bash {"command": "echo a > old.txt; touch -d '1 hour ago' old.txt"}
tool write {"path": "old.txt", "content": "b\n"}
say done
