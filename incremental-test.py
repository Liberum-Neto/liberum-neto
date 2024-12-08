import time
import matplotlib.pyplot as plt
import numpy as np
import os
import subprocess

CORE_BIN = "./target/release/liberum_core"
CLI_BIN = "./target/release/liberum_cli"
INIT_COUNT = 5
NODE_COUNT = 25
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
FIND_TIMES=[]
for i in range (1, NODE_COUNT) :
    N = "test_n" + str(i)
    N_ADDR = NODE_ADDR_PREFIX + str(i + 52136) + NODE_ADDR_SUFFIX

    subprocess.run([CLI_BIN, "-d", "new-node", N, "--id-seed", str(i)])
    subprocess.run([CLI_BIN, "-d", "config-node", N, "add-external-addr", N_ADDR])
    if i > 0:
        subprocess.run([CLI_BIN, "config-node", N, "add-bootstrap-node", N_IDS[i-1], N_ADDRESSES[i-1]])

    subprocess.run([CLI_BIN, "-d", "start-node", N])

    ID = subprocess.run([CLI_BIN, "-d", "get-peer-id", N], stdout=subprocess.PIPE).stdout.decode().strip()
    N_NAMES.append(N)
    N_IDS.append(ID)
    N_ADDRESSES.append(N_ADDR)
    time.sleep(0.01)
    if i == INIT_COUNT:
        FILE_ID=subprocess.run([CLI_BIN, "publish-file", N_NAMES[0], FILE_NAME], stdout=subprocess.PIPE).stdout.decode().strip()
    if i > INIT_COUNT:
        RESULTS.append([])
        FIND_TIMES.append(0)

        for j in range(1, i):
            t0 = time.time()
            RESULT=subprocess.run([CLI_BIN, "-d", "download-file", N_NAMES[j], FILE_ID], stdout=subprocess.PIPE).stdout.decode().strip()
            t = time.time()-t0
            cmp = FILE_CONTENT == RESULT
            RESULTS[-1].append(cmp)
            FIND_TIMES[-1] += t
        FIND_TIMES[-1] = (FIND_TIMES[-1] / i) * 1000.0

for x in range(0, len(RESULTS)):
    data = [i for i in range(len(RESULTS[x])) if RESULTS[x][i] == True]
    X = [x] * len(data)
    plt.scatter(X, data)
plt.title("Udane wyszukania, " + str(INIT_COUNT) + " węzłów w sieci w chwili publikacji")
plt.ylabel("Numer węzła")
plt.xlabel("Ilość nowych węzłów dodanych do sieci")
plt.show()

failed_counts = []
for x in range(0, len(RESULTS)):
    data = [i for i in range(len(RESULTS[x])) if RESULTS[x][i] == True]
    failed = len(data) - len(RESULTS[x])
    failed_counts.append(failed)
plt.plot(failed_counts)
plt.title("Liczba nieudanch wyszukań, " + str(INIT_COUNT) + " węzłów w sieci w chwili publikacji")
plt.ylabel("iczba nieudanch wyszukań")
plt.xlabel("Ilość nowych węzłów dodanych do sieci")
plt.show()

plt.plot(FIND_TIMES)
plt.title("Średni czas odnajdywania, " + str(INIT_COUNT) + " węzłów w sieci w chwili publikacji")
plt.ylabel("Czas [ms]")
plt.xlabel("Ilość nowych węzłów dodanych do sieci")
plt.show()

for i in range(0, NODE_COUNT):
    subprocess.run([CLI_BIN, "stop-node", N_NAMES[i]])

subprocess.run(["killall", "liberum_core"])
