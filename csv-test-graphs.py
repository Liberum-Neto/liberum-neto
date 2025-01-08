import pandas as pd
import matplotlib.pyplot as plt


df = pd.read_csv('data.csv', sep=';')
downloads = df.loc[df['ActionResultType'] == 71]

parts = downloads['TestInstancePartId'].unique()
x = [downloads.loc[downloads['TestInstancePartId'] == part] for part in parts]
successes = [x.loc[x['IsSuccess'] == True]['IsSuccess'].count() for x in x]
times_ms = [x.loc[x['IsSuccess'] == True]['QueryDurationInNano'].mean() / 1_000_000_000.0 for x in x]
times_min = [x.loc[x['IsSuccess'] == True]['QueryDurationInNano'].min() / 1_000_000_000.0 for x in x]
times_max = [x.loc[x['IsSuccess'] == True]['QueryDurationInNano'].max() / 1_000_000_000.0 for x in x]
requests = [x.loc[x['IsSuccess'] == True]['TotalRequest'].mean() for x in x]
fails = [x.loc[x['IsSuccess'] == False]['IsSuccess'].count() for x in x]
percentage = [fails[i] / (successes[i] + fails[i]) * 100 for i in range(len(parts))]

points = []
for part in parts:
    part_data = downloads.loc[downloads['TestInstancePartId'] == part]
    points.extend([part, nid] for nid in part_data['NodeId'].unique() if part_data.loc[part_data['NodeId'] == nid]['IsSuccess'].any() == True)

plt.title("Pobrania")
plt.scatter(parts, successes, label='Udane')
plt.scatter(parts, fails, label='Nieudane')
plt.ylabel('Liczba')
plt.xlabel('Liczba dodanych do sieci węzłów')
plt.legend()
plt.savefig('Pobrania-rzeczywiste.png', dpi=300)
plt.show()

plt.title("Procent nieudanych pobrań")
plt.scatter(parts, percentage, label='Procent nieudanych pobrań')
plt.ylabel('Nieudane pobrania [%]')
plt.xlabel('Liczba dodanych do sieci węzłów')
plt.legend()
plt.savefig('Procent-pobrania-rzeczywiste.png', dpi=300)
plt.show()

plt.scatter([x[0] for x in points], [x[1] for x in points], s=5)
plt.title("Udane pobrania konkretnych węzłów")
plt.ylabel('Numer węzła')
plt.xlabel('Liczba dodanych do sieci węzłów')
plt.savefig('Pobrania-węzły.png', dpi=300)
plt.show()

plt.title("Wydajność pobierania")
plt.scatter(parts, requests, label='Średnia liczba zapytań', marker="x", c='blue')
plt.scatter(parts, times_min, label='Minimalny czas pobierania [s]', marker="v", c='red')
plt.scatter(parts, times_ms, label='Średni czas pobierania [s]', c='orange')
plt.scatter(parts, times_max, label='Maksymalny czas pobierania [s]', marker="^", c='green')
plt.ylabel('Wartość')
plt.xlabel('Liczba dodanych do sieci węzłów')
plt.legend(loc='upper left')
plt.gcf().set_size_inches(10, 5)
plt.savefig('Wydajność-pobierania.png', dpi=300)
plt.show()
