tool read {"path": "shared.txt"}
slow 1500 tool edit {"path": "shared.txt", "old": "draft", "new": "A's draft"} tool edit {"path": "shared.txt", "old": "draft\n", "new": "draft, by A\n"}
tool edit {"path": "shared.txt", "old": "draft", "new": "A's draft"}
tool edit {"path": "shared.txt", "old": "changed by B", "new": "changed by B, then A"}
tool bash {"command": "cat other.txt"}
tool write {"path": "other.txt", "content": "mine\n"}
tool write {"path": "other.txt", "content": "mine\n"}
tool read {"path": "f.txt"}
tool edit {"path": "f.txt", "old": "one", "new": "ONE"} tool edit {"path": "f.txt", "old": "two", "new": "TWO"}
tool edit {"path": "f.txt", "old": "ONE", "new": "1"} tool bash {"command": "sed -i s/TWO/2/ f.txt"} tool edit {"path": "f.txt", "old": "1", "new": "uno"}
tool bash {"command": "echo a > new.txt"}
tool write {"path": "new.txt", "content": "b\n"}
tool bash {"command": "seq 1 20000 > big.txt; printf 'x\\0y' > blob.bin; touch -d '1 hour ago' big.txt blob.bin"}
tool edit {"path": "big.txt", "old": "\n15000\n", "new": "\nfifteen thousand\n"}
tool edit {"path": "big.txt", "old": "\n15000\n", "new": "\nfifteen thousand\n"}
tool write {"path": "blob.bin", "content": "z"}
tool read {"path": "f.txt"} tool bash {"command": "rm f.txt"}
tool write {"path": "f.txt", "content": "fresh\n"}
say done
