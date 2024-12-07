import time
import matplotlib.pyplot as plt
import numpy as np
import os
import subprocess

CORE_BIN = "./target/debug/liberum_core"
CLI_BIN = "./target/debug/liberum_cli"
NODE_COUNT = 20
FILE_NAME = os.path.dirname(os.path.realpath(__file__)) + "/test-file.txt"
FILE_CONTENT = "Hello, World!"

NODE_ADDR_PREFIX = "/ip6/::1/udp/"
NODE_ADDR_SUFFIX = "/quic-v1"

subprocess.run(["killall", "liberum_core"])
os.system(CORE_BIN + "$CORE_BIN --daemon  &> /dev/null &")
time.sleep(0.5)

# create nodes
N_NAMES=[]
N_IDS=[]
N_ADDRESSES=[]

N = "test_n0"
N_ADDR = NODE_ADDR_PREFIX + str(52136) + NODE_ADDR_SUFFIX

subprocess.run([CLI_BIN, "-d", "new-node", N, "--id-seed", "0"])
subprocess.run([CLI_BIN, "-d", "config-node", N, "add-external-addr", N_ADDR])

subprocess.run([CLI_BIN, "-d", "start-node", N])

ID = subprocess.run([CLI_BIN, "-d", "get-peer-id", N], stdout=subprocess.PIPE).stdout.decode().strip()
N_NAMES.append(N)
N_IDS.append(ID)
N_ADDRESSES.append(N_ADDR)

# create and provide file
with open(FILE_NAME, mode="w") as f:
    f.write(FILE_CONTENT)


RESULTS=[]
for i in range (1, NODE_COUNT) :
    N = "test_n" + str(i)
    N_ADDR = NODE_ADDR_PREFIX + str(i + 52136) + NODE_ADDR_SUFFIX

    subprocess.run([CLI_BIN, "-d", "new-node", N, "--id-seed", str(i)])
    subprocess.run([CLI_BIN, "-d", "config-node", N, "add-external-addr", N_ADDR])
    if i > 0:
        subprocess.run([CLI_BIN, "config-node", N, "add-bootstrap-node", N_IDS[i -1], N_ADDRESSES[i-1]])

    subprocess.run([CLI_BIN, "-d", "start-node", N])

    ID = subprocess.run([CLI_BIN, "-d", "get-peer-id", N], stdout=subprocess.PIPE).stdout.decode().strip()
    N_NAMES.append(N)
    N_IDS.append(ID)
    N_ADDRESSES.append(N_ADDR)
    time.sleep(0.2)
    if i == 3:
        FILE_ID=subprocess.run([CLI_BIN, "publish-file", N_NAMES[0], FILE_NAME], stdout=subprocess.PIPE).stdout.decode().strip()
    if i > 3:
        RESULTS.append([])

        for j in range(0, i):
            RESULT=subprocess.run([CLI_BIN, "-d", "download-file", N_NAMES[i], FILE_ID], stdout=subprocess.PIPE).stdout.decode().strip()
            cmp = FILE_CONTENT == RESULT
            print(FILE_CONTENT, RESULT)
            print(cmp)
            RESULTS[i-4].append(cmp)

for x in range(0, len(RESULTS)):
    data = [i for i in range(len(RESULTS[x])) if RESULTS[x][i] == True]
    X = [x] * len(data)
    plt.scatter(X, data)
plt.show()

for i in range(0, NODE_COUNT):
    subprocess.run([CLI_BIN, "stop-node", N_NAMES[i]])

subprocess.run(["killall", "liberum_core"])
print(RESULTS)
